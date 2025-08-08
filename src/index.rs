use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    path,
    str::FromStr,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::blob::Blob;

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
        // Check for index file's existence. If not there, then create anew and return empty Index.
        let index_file = path::PathBuf::from_str(".gitlet/index")
            .with_context(|| "Create PathBuf for index file")
            .unwrap();

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
        let f = std::fs::File::create(".gitlet/index")
            .with_context(|| "Create .gitlet/index file")
            .unwrap();

        serde_json::to_writer(f, &self).context("Save staging area to .gitlet/index")?;

        Ok(())
    }

    /// Clears the index file and drops the Index
    fn clear(self) -> Result<()> {
        let _res = std::fs::remove_file(".gitlet/index").context("Delete .gitlet/index")?;
        Ok(())
    }

    fn stage(&mut self, f: path::PathBuf) -> Result<()> {
        let blob = Blob::new(&f)
            .with_context(|| "Creating blob for addition to index")
            .unwrap();

        self.additions.insert(f, blob);

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

pub fn add(f: &str) -> Result<()> {
    let mut index = Index::load()?;

    let f = path::PathBuf::from(f);
    anyhow::ensure!(f.exists(), "Cannot stage file. File does not exist.");

    index.stage(f).with_context(|| "Staging file")?;

    index
        .save()
        .with_context(|| "Saving the staging area to the index file")?;

    Ok(())
}

#[cfg(test)]
mod tests {
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

            let tmp = assert_fs::NamedTempFile::new("tmp.txt")?;
            tmp.write_str("Test text.")?;

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

        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir_all(".gitlet/blobs")?;

            let tmp = assert_fs::NamedTempFile::new("tmp.txt")?;
            tmp.write_str("Test text.")?;

            assert!(action(IndexAction::Add, tmp.to_str().unwrap()).is_ok());
            assert!(action(IndexAction::Unstage, tmp.to_str().unwrap()).is_ok());

            let index = Index::load()?;
            assert!(index.additions.is_empty());

            Ok(())
        })
    }
}
