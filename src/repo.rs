use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;

/// Represents a gitlet repository. This module provides methods for creating a new repository
/// and for interacting with an existing one.
// pub struct Repo {}

/// Initializes a new gitlet repository. `repo_path` is an optional argument passed to
/// `gitlet init` to specify the directory for the new repository. It defaults to the PWD.
pub fn init(repo_dir: Option<String>) -> Result<()> {
    // If a repository directory was provided, then convert it to a Path,
    // otherwise, use the PWD.
    let repo_dir = match repo_dir {
        Some(dir) => dir.clone(),
        None => ".".to_string(),
    };
    let rpath = Path::new(&repo_dir);

    if rpath.join(".gitlet").exists() {
        return Err(anyhow!(
            "A gitlet repository already exists in this directory"
        ));
    }

    if !rpath.exists() {
        fs::create_dir(rpath).expect("Failed to create directory for repository");
    }

    fs::create_dir(rpath.join(".gitlet"))?;
    fs::create_dir(rpath.join(".gitlet/blobs"))?;
    fs::create_dir(rpath.join(".gitlet/commits"))?;
    fs::create_dir(rpath.join(".gitlet/refs"))?;
    fs::File::create(rpath.join(".gitlet/HEAD"))?;

    println!("Initialized empty Gitlet repository");

    Ok(())
}
/// Returns the given filepath relative to the root directory of the working tree.
///
/// For example, given a path `/var/tmp/work/sub/t.rs`, and assuming `/var/tmp/work/.gitlet`, the
/// function would return "sub/t.rs".
///
/// It returns an error if there is no Gitlet repository.
///
/// This is useful for nested directory structures as well as for stripping arbitrary parent paths,
/// such as with absolute paths.
pub fn find_working_tree_dir(filepath: &std::path::Path) -> Result<PathBuf> {
    let filepath = std::fs::canonicalize(&filepath).with_context(|| {
        format!(
            "Creating absolute path for filepath: '{}'",
            filepath.to_str().unwrap()
        )
    })?;

    // Find the root of the Gitlet repository.
    let mut curr_dir = std::env::current_dir()?.join(&filepath.file_name().unwrap());
    let mut found = false;

    while curr_dir.pop() {
        for entry in curr_dir
            .read_dir()
            .expect("read_dir: entry in absolute path")
        {
            if let Ok(entry) = entry {
                if entry.file_name() == ".gitlet" {
                    found = true;
                    break;
                }
            }
        }
        if found {
            break;
        }
    }

    anyhow::ensure!(found, "Not a valid gitlet repository");

    // At this point, curr_dir contains the absolute path to the gitlet repository's root working
    // tree directory.

    let relative_path = filepath
        .strip_prefix(&curr_dir)
        .context("Strip absolute path of prefix")?;

    Ok(relative_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;

    #[test]
    fn create_rel_path_from_repo_root() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;

        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir(".gitlet")?;
            std::fs::File::create("t.txt")?;

            let fp = std::path::Path::new("t.txt");
            let res = find_working_tree_dir(fp)?;

            assert_eq!(res.as_os_str(), "t.txt");

            Ok(())
        })
    }

    #[test]
    fn create_rel_path_from_depth_one() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;

        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir(".gitlet")?;
            std::fs::create_dir("a")?;
            std::fs::File::create("a/t.txt")?;

            std::env::set_current_dir("a").context("set current dir to 'tmpdir/a/'")?;
            let fp = std::path::Path::new("t.txt");
            let res = find_working_tree_dir(fp)?;

            assert_eq!(res.as_os_str(), "a/t.txt");

            Ok(())
        })
    }

    #[test]
    fn no_gitlet_dir() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;

        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir("a")?;
            std::fs::File::create("a/t.txt")?;

            std::env::set_current_dir("a").context("set current dir to 'tmpdir/a/'")?;
            let fp = std::path::Path::new("t.txt");
            let res = find_working_tree_dir(fp);

            assert!(res.is_err());

            Ok(())
        })
    }
}
