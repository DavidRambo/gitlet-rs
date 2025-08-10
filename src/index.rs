use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path,
    str::FromStr,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{blob::Blob, repo};

#[derive(Default, Deserialize, Serialize)]
struct Index {
    additions: HashMap<path::PathBuf, Blob>,
    removals: HashSet<path::PathBuf>,
}

pub enum IndexAction {
    Add,
    Remove,
    Unstage,
}

impl Index {
    /// Loads the staging area from .gitlet/index
    fn load() -> Result<Self> {
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

    /// Clears the index file and drops the Index
    fn clear(self) -> Result<()> {
        let index_file = repo::abs_path_to_repo_root()?.join(".gitlet/index");
        std::fs::remove_file(index_file).context("Delete .gitlet/index")?;
        Ok(())
    }

    /// Stages a file for addition in the next commit.
    fn stage(&mut self, filepath: path::PathBuf, fpath_from_root: path::PathBuf) -> Result<()> {
        let blob = Blob::new(&filepath).with_context(|| "Creating blob for addition to index")?;
        blob.write_blob(&filepath)?;

        self.additions.insert(fpath_from_root, blob);

        self.save()
    }

    // FIX: Need first to check that the file is tracked. Note that this is a Git thing and not in
    // the Gitlet spec.
    fn remove(&mut self, f: &path::Path) -> Result<()> {
        self.additions.remove(f);
        let res = self.removals.insert(f.to_path_buf());
        anyhow::ensure!(res, "Failed to stage file for removal");
        self.save()
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

        write!(f, "{}", buf)
    }
}

pub fn status(mut writer: impl std::io::Write) -> Result<()> {
    let index = Index::load()?;
    write!(writer, "{index}").expect("Failed to write Index");
    Ok(())
}

/// Dispatches gitlet command, passed as IndexAction.
pub fn action(action: IndexAction, filepath: &str) -> Result<()> {
    let mut index = Index::load()?;

    let f = path::PathBuf::from(filepath);
    anyhow::ensure!(f.exists(), "Cannot stage file. File does not exist.");

    let fpath_from_root = repo::find_working_tree_dir(&f)
        .with_context(|| "Convert filepath to be relative to working tree root")?;

    match action {
        IndexAction::Add => index
            .stage(f, fpath_from_root)
            .with_context(|| "Staging file")?,
        IndexAction::Remove => {
            index.additions.remove(&fpath_from_root);
            index.removals.insert(fpath_from_root);
        }
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

            new_index.clear()?;

            let renew_index = Index::load()?;
            assert!(renew_index.additions.is_empty());
            assert!(renew_index.removals.is_empty());
            Ok(())
        })
    }

    #[test]
    fn stage_for_removal() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;

        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir_all(".gitlet/blobs")?;

            let mut f = std::fs::File::create("tmp.txt")?;
            f.write_all(b"Test text.")?;
            let tmp = path::PathBuf::from_str("tmp.txt")?;

            assert!(action(IndexAction::Remove, tmp.to_str().unwrap()).is_ok());

            let index = Index::load()?;
            assert!(index.additions.is_empty());
            assert!(index.removals.contains(&tmp.to_path_buf()));

            Ok(())
        })
    }

    #[test]
    fn stage_and_unstage() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;
        dbg!(&tmpdir);

        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir_all(".gitlet/blobs")?;

            let mut f = std::fs::File::create("tmp.txt")?;
            f.write_all(b"Test text.")?;
            let tmp = path::PathBuf::from_str("tmp.txt")?;

            assert!(action(IndexAction::Add, tmp.to_str().unwrap()).is_ok());
            assert!(action(IndexAction::Unstage, tmp.to_str().unwrap()).is_ok());

            let index = Index::load()?;
            assert!(index.additions.is_empty());

            Ok(())
        })
    }
}
