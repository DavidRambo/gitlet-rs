use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    blob::Blob,
    repo::{self, abs_path_to_repo_root},
};

#[derive(Default, Deserialize, Serialize)]
pub(crate) struct Index {
    pub(crate) additions: HashMap<path::PathBuf, Blob>,
    pub(crate) removals: HashSet<path::PathBuf>,
}

pub enum IndexAction {
    Add,
    Unstage,
}

impl Index {
    /// Loads the staging area from .gitlet/index
    pub(crate) fn load() -> Result<Self> {
        let index_file = repo::abs_path_to_repo_root()?.join(".gitlet/index");

        // Check for index file's existence. If not there, then create anew and return empty Index.
        if !index_file.exists() {
            let index = Self::default();
            index.save()?; // save() creates/truncates the index file
            Ok(index)
        } else {
            let mut f = std::fs::File::open(&index_file).context("Open .gitlet/index")?;
            let stat = std::fs::metadata(&index_file).context("Stat .gitlet/index")?;
            let mut content = String::with_capacity(stat.len() as usize);
            f.read_to_string(&mut content)?;
            let staging_area: Index = serde_json::from_str(&content)
                .context("Open .gitlet/index and deserialize into Index")?;

            Ok(staging_area)
        }
    }

    /// Saves the staging area to .gitlet/index
    fn save(&self) -> Result<()> {
        let index_file = repo::abs_path_to_repo_root()?.join(".gitlet/index");
        let f = std::fs::File::create(index_file)
            .with_context(|| "Create .gitlet/index file")
            .unwrap();

        serde_json::to_writer(f, &self).context("Save staging area to .gitlet/index")?;

        Ok(())
    }

    /// Stages a file for addition in the next commit.
    fn stage(&mut self, filepath: path::PathBuf, fpath_from_root: path::PathBuf) -> Result<()> {
        let blob = Blob::new(&filepath).with_context(|| "Creating blob for addition to index")?;
        blob.save(&filepath)?;

        self.removals.remove(&fpath_from_root);
        self.additions.insert(fpath_from_root, blob);

        self.save()
    }

    /// Returns true if the staging area is clear.
    pub(crate) fn is_clear(&self) -> bool {
        self.additions.is_empty() && self.removals.is_empty()
    }
}

impl std::fmt::Display for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::new();
        buf.push_str("=== Staged Files ===\n");
        for filename in self.additions.keys() {
            buf.push_str(filename.to_str().unwrap());
            buf.push('\n');
        }

        buf.push_str("\n=== Removed Files ===\n");
        for filename in self.removals.iter() {
            buf.push_str(filename.to_str().unwrap());
            buf.push('\n');
        }

        write!(f, "{buf}")
    }
}
/// Clears the index file without needing the Index
pub(crate) fn clear_index() -> Result<()> {
    let index_file = repo::abs_path_to_repo_root()?.join(".gitlet/index");
    if index_file.exists() {
        std::fs::remove_file(index_file).context("Delete .gitlet/index")?;
    }
    Ok(())
}

/// Displays the files staged for addition and for removal.
pub fn status(mut writer: impl std::io::Write) -> Result<()> {
    let index = Index::load()?;
    write!(writer, "{index}").expect("Failed to write Index");
    Ok(())
}

/// Dispatches gitlet command either to stage or unstage a file.
pub fn action(action: IndexAction, filepath: &str) -> Result<()> {
    let mut index = Index::load()?;

    let f = path::PathBuf::from(filepath);
    anyhow::ensure!(f.exists(), "Cannot stage file. File does not exist.");

    let fpath_from_root = repo::find_working_tree_dir(&f)
        .with_context(|| "Convert filepath to be relative to working tree root")?;

    match action {
        IndexAction::Add => index.stage(f, fpath_from_root).context("Stage file")?,
        IndexAction::Unstage => {
            index.additions.remove(&fpath_from_root);
            index.removals.remove(&fpath_from_root);
        }
    }

    index
        .save()
        .with_context(|| "Saving the staging area to the index file")?;

    Ok(())
}

/// Removes file from the working tree and stages it for removal, or, if 'cached' is true, then
/// only untracks the file.
pub fn rm(cached: bool, file_name: &str) -> Result<()> {
    let f = path::Path::new(file_name);

    // In case deleted file needs to be unstaged and/or untracked.
    if !f.exists() {
        return rm_deleted(f);
    }

    let fpath_from_root =
        repo::find_working_tree_dir(f).context("Create filepath from repo root")?;

    let mut index = Index::load().context("Load index")?;

    // Check whether file is tracked.
    if !index.additions.contains_key(&fpath_from_root)
        && !repo::is_tracked_by_head(&fpath_from_root)
    {
        println!("Cannot remove file. The file is not tracked.");
        return Ok(());
    }

    if cached {
        // Remove from index.
        if index.additions.remove(&fpath_from_root).is_some() {
            // Remove the staged blob. If it is added again, then the working tree will be the source.
            // Note: git keeps the blob (perhaps it prunes things periodically?)
            let blob = Blob::new(&fpath_from_root)?;
            blob.delete()?;
        }

        index.save().context("Save staging area to index")?;
    } else {
        // Per git, cannot rm a file that has changes staged for commit.
        if index.additions.contains_key(&fpath_from_root) {
            anyhow::bail!("Cannot remove a file with staged changes. Use --cached to unstage.");
        }

        // Remove from the working tree.
        std::fs::remove_file(file_name).context("Delete file from working tree")?;
    }

    // Only need to stage for removal if it is tracked by the current commit.
    if repo::is_tracked_by_head(&fpath_from_root) {
        index.removals.insert(fpath_from_root);
        index
            .save()
            .context("Save index after inserting to removals")?;
    }

    println!("rm '{file_name}'");

    Ok(())
}

/// Updates the staging area, if necessary, to reflect the removal of a deleted file.
fn rm_deleted(f: &path::Path) -> Result<()> {
    let abs_fp = path::absolute(f).context("Create absolute path to file name")?;
    let repo_root = abs_path_to_repo_root().context("Get absolute path to repo root dir")?;
    let repo_file = abs_fp
        .strip_prefix(&repo_root)
        .context("Strip absolute path prefix")?;

    let mut index = Index::load().context("Load index")?;

    // Stop if file is not tracked.
    if !index.additions.contains_key(repo_file) && !repo::is_tracked_by_head(repo_file) {
        anyhow::bail!("Cannot remove file. The file is not tracked.");
    }
    // File is _either_ staged for addition _or_ tracked; it _could_ be in removals.

    // If it is not already in removals and is also tracked, then stage it for removal.
    if !index.removals.contains(repo_file) && repo::is_tracked_by_head(repo_file) {
        index.removals.insert(repo_file.to_path_buf());
        println!("Staged file for removal");
    } else if index.additions.remove(repo_file).is_some() {
        println!("Removed deleted file from staging area.");
    } else {
        index.save()?;
        anyhow::bail!("File already staged for removal.");
    }
    index.save()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use assert_fs::prelude::FileWriteStr;

    use crate::test_utils;

    use super::*;

    #[test]
    fn load_empty_staging_area() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;

        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir_all(".gitlet/blobs")?;

            let new_index = Index::load()?;
            assert!(new_index.additions.is_empty());
            assert!(new_index.removals.is_empty());

            Ok(())
        })
    }

    #[test]
    fn clear_staging_area() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;

        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir_all(".gitlet/blobs")?;

            let tmp = assert_fs::NamedTempFile::new("tmp.txt")?;
            tmp.write_str("Test text.")?;

            let mut new_index = Index::load()?;
            new_index.additions = HashMap::new();
            new_index
                .additions
                .insert(tmp.to_path_buf(), Blob::new(&tmp)?);

            let _ = clear_index();

            let renew_index = Index::load()?;
            assert!(renew_index.additions.is_empty());
            assert!(renew_index.removals.is_empty());
            Ok(())
        })
    }

    #[test]
    fn stage_and_unstage() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;

        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir_all(".gitlet/blobs")?;

            let mut f = std::fs::File::create("tmp.txt")?;
            f.write_all(b"Test text.")?;
            let tmp = path::PathBuf::from("tmp.txt");

            assert!(action(IndexAction::Add, tmp.to_str().unwrap()).is_ok());
            assert!(action(IndexAction::Unstage, tmp.to_str().unwrap()).is_ok());

            let index = Index::load()?;
            assert!(index.additions.is_empty());

            Ok(())
        })
    }

    #[test]
    fn test_rm_staged() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;

        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir_all(".gitlet/blobs")?;

            let mut f = std::fs::File::create("tmp.txt")?;
            f.write_all(b"Test text.")?;
            let tmp = path::PathBuf::from("tmp.txt");

            assert!(action(IndexAction::Add, tmp.to_str().unwrap()).is_ok());

            assert!(rm(true, tmp.to_str().unwrap()).is_ok());

            let index = Index::load()?;
            assert!(index.additions.is_empty());
            assert!(!index.removals.contains(&tmp.to_path_buf()));
            assert!(std::fs::exists("tmp.txt")?);

            Ok(())
        })
    }
}
