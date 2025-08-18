//! This module provides methods for creating a new repository and for interacting with an existing one.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{self, Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use walkdir::WalkDir;

use crate::commit::{Commit, get_commit_blobs};
use crate::index::{self, Index};

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

    fs::create_dir(rpath.join(".gitlet")).context("Create '.gitlet/'")?;
    fs::create_dir(rpath.join(".gitlet/blobs")).context("Create '.gitlet/blobs/'")?;
    fs::create_dir(rpath.join(".gitlet/commits")).context("Create '.gitlet/commits/'")?;
    fs::create_dir(rpath.join(".gitlet/refs")).context("Create '.gitlet/refs/'")?;
    fs::File::create(rpath.join(".gitlet/refs/main")).context("Create '.gitlet/refs/main'")?;
    let mut head = fs::File::create(rpath.join(".gitlet/HEAD")).context("Create '.gitlet/HEAD'")?;
    head.write_all(b"main")
        .context("Write 'main' to '.gitlet/HEAD'")?;

    println!("Initialized empty Gitlet repository");

    Ok(())
}

/// Prints the status of the gitlet repository to stdout.
pub fn status() -> Result<()> {
    let stdout = io::stdout();
    let handle = stdout.lock();
    let mut buf_handle = io::BufWriter::new(handle);

    let branch_name = read_head_branch()?;
    writeln!(buf_handle, "On branch {branch_name}\n")?;

    // Staged for addition and for removal
    index::status(&mut buf_handle)?;

    writeln!(buf_handle, "\n=== Unstaged Modifications ===")?;
    let unstaged = unstaged_modifications().context("Collect unstaged modified files")?;
    for entry in unstaged {
        writeln!(buf_handle, "{}", &entry)?;
    }

    writeln!(buf_handle, "\n=== Untracked Files ===")?;
    for entry in untracked_files().context("Collect untracked files in working tree")? {
        writeln!(buf_handle, "{}", &entry.display())?;
    }

    writeln!(buf_handle)?;

    buf_handle.flush()?;

    Ok(())
}

/// Returns the given filepath relative to the root directory of the working tree.
///
/// For example, given a path `/var/tmp/work/sub/t.rs`, and assuming `/var/tmp/work/.gitlet`, the
/// function would return "sub/t.rs".
///
/// # Panics
/// It returns an error if there is no Gitlet repository or if the filepath does not exist.
///
/// This is useful for nested directory structures as well as for stripping arbitrary parent paths,
/// such as with absolute paths.
pub(crate) fn find_working_tree_dir(filepath: &path::Path) -> Result<PathBuf> {
    let filepath = std::fs::canonicalize(filepath).with_context(|| {
        format!(
            "Creating absolute path for filepath: '{}'",
            filepath.display()
        )
    })?;

    // Find the root of the Gitlet repository.
    let curr_dir = abs_path_to_repo_root()?;

    let relative_path = filepath
        .strip_prefix(&curr_dir)
        .context("Strip absolute path of prefix")?;

    Ok(relative_path.to_path_buf())
}

/// Returns the absolute path to the root of the working tree in which the .gitlet/ directory resides.
pub(crate) fn abs_path_to_repo_root() -> Result<PathBuf> {
    let curr_dir = std::env::current_dir().context("Get current working directory")?;
    let mut curr_dir = curr_dir.join("dummy_file_to_pop");
    let mut found = false;

    while !found && curr_dir.pop() {
        for entry in curr_dir
            .read_dir()
            .expect("read_dir: entry in absolute path")
            .flatten()
        {
            if entry.file_name() == ".gitlet" {
                found = true;
                break;
            }
        }
    }

    anyhow::ensure!(found, "Not a valid gitlet repository");

    Ok(curr_dir)
}

/// Returns the absolute path of the file in the working tree.
fn abs_path_working_file(fp: &path::Path) -> Result<path::PathBuf> {
    let mut repo_root = abs_path_to_repo_root()?;
    repo_root.push(fp);
    Ok(repo_root)
}

/// Commits the staged changes to the repository.
pub fn commit(message: String) -> Result<()> {
    let index = index::Index::load().context("Load index for commit")?;
    if index.is_clear() {
        println!("Nothing to commit.");
        return Ok(());
    }

    // Get the parent commit hash.
    let parent_hash =
        read_head_hash().context("Retrieve current commit hash for parent of new commit")?;

    let new_commit = Commit::new(parent_hash, None, message, index).context("Create commit")?;
    update_head(&new_commit.hash)?;
    new_commit.save().context("Save new commit to repository")?;

    index::clear_index().context("Clear the staging area")?;

    Ok(())
}

/// Helper function to update HEAD file
fn update_head(hash: &str) -> Result<()> {
    let repo_root = abs_path_to_repo_root().context("Get absolute path to repo root")?;
    let mut head = std::fs::File::open(repo_root.join(".gitlet/HEAD")).context("Open HEAD file")?;

    let mut branch_name = String::new();
    head.read_to_string(&mut branch_name)
        .context("Read branch name from HEAD")?;

    let mut branch_ref = std::fs::File::create(repo_root.join(".gitlet/refs").join(branch_name))
        .context("Truncate branch ref file")?;
    branch_ref
        .write_all(hash.as_bytes())
        .context("Write hash to HEAD")?;
    Ok(())
}

/// Get the name of the branch in HEAD
fn read_head_branch() -> Result<String> {
    let repo_root = abs_path_to_repo_root().context("Get absolute path to repo root")?;
    let mut head = std::fs::File::open(repo_root.join(".gitlet/HEAD")).context("Open HEAD file")?;

    let mut branch_name = String::new();
    head.read_to_string(&mut branch_name)
        .context("Read branch name from HEAD")?;

    Ok(branch_name)
}

/// Returns true if the given file is tracked.
///
/// A file is tracked if it is represented either by the HEAD commit or by the index.
pub(crate) fn is_tracked_by_head(filepath: &Path) -> bool {
    let Ok(head_commit) = retrieve_head_commit() else {
        return false;
    };

    head_commit.tracks(filepath)
}

fn read_head_hash() -> Result<String> {
    let repo_root = abs_path_to_repo_root()?;

    let branch_name = std::fs::read_to_string(repo_root.join(".gitlet/HEAD"))
        .context("Read branch name from HEAD")?;

    let branch_ref = std::fs::read_to_string(repo_root.join(".gitlet/refs").join(branch_name))
        .context("Read current HEAD commit")?;

    if branch_ref.len() != 0 && branch_ref.len() != 40 {
        anyhow::bail!("Invalid commit");
    }

    Ok(branch_ref)
}

/// Returns the commit referenced by the HEAD file's hash.
fn retrieve_head_commit() -> Result<Commit> {
    Commit::load(&read_head_hash()?)
}

/// Prints out a log of the commit history starting from the HEAD.
pub fn log() -> Result<()> {
    let head_commit = retrieve_head_commit().context("Retrieve head commit for log")?;
    for c in head_commit.iter() {
        println!("{c}");
    }
    Ok(())
}

/// Returns all non-hidden filepaths in the working tree.
///
/// Snippet to skip hidden files: https://docs.rs/walkdir/latest/walkdir/#example-skip-hidden-files-and-directories-on-unix
fn working_files() -> Result<Vec<PathBuf>> {
    let repo_root = abs_path_to_repo_root().context("Get repository root directory")?;
    let all_files = WalkDir::new(&repo_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && e.file_name()
                    .to_str()
                    .map(|s| !s.starts_with("."))
                    .unwrap_or(false)
        })
        .map(|e| PathBuf::from(e.path().strip_prefix(&repo_root).unwrap()))
        .collect();

    Ok(all_files)
}

/// Returns names of files that are tracked (either by the HEAD or by the index) and have been
/// changed but not staged, including deleted files, which are marked as such.
fn unstaged_modifications() -> Result<Vec<String>> {
    let mut unstaged: Vec<String> = Vec::new();

    // Iterate through all tracked files in the working tree, comparing current hash with both HEAD
    // and index.
    let working_files = working_files().context("Collect filepaths in working tree")?;
    let index = Index::load().context("Load index")?;

    for (f, tracked_blob) in get_commit_blobs(&read_head_hash()?)
        .context("Get HEAD commit's list of tracked files")?
        .iter()
    {
        // If file is in neither the working tree nor staged removals, then it has been deleted.
        if !working_files.contains(f) && !index.removals.contains(f) {
            let mut deleted_file = String::from(f.to_str().unwrap());
            deleted_file.push_str(" (deleted)");
            unstaged.push(deleted_file);
        } else if working_files.contains(f) {
            // Render the filepath to be absolute.
            let abs_fpath = abs_path_working_file(f).context("Create absolute path to file")?;

            // Compare first to the index, in case the changes have already been staged.
            // Then compare to last commited blob.
            if index.additions.contains_key(f)
                && !index
                    .additions
                    .get(f)
                    .unwrap()
                    .hash_same_as_other_file(&abs_fpath)
                    .unwrap_or(false)
            {
                // File has been staged for addition and subsequently changed.
                unstaged.push(String::from(f.to_str().unwrap()));
            } else if !index.additions.contains_key(f)
                && !tracked_blob
                    .hash_same_as_other_file(&abs_fpath)
                    .context("Compare current file to recent commit version")?
            {
                // File has been modified but not staged for addition.
                unstaged.push(String::from(f.to_str().unwrap()));
            }
        }
    }

    // Iterate through all new files staged for addition, pushing those that have since been
    // modified.
    for (f, staged_blob) in index
        .additions
        .iter()
        .filter(|(k, _)| !is_tracked_by_head(k))
    {
        if !working_files.contains(f) {
            let mut deleted_file = String::from(f.to_str().unwrap());
            deleted_file.push_str(" (deleted)");
            unstaged.push(deleted_file);
        } else if !staged_blob.hash_same_as_other_file(f).unwrap_or(true) {
            unstaged.push(String::from(f.to_str().unwrap()));
        }
    }

    Ok(unstaged)
}

/// Returns filepaths in the working tree that are not tracked by the currently checked out commit.
fn untracked_files() -> Result<Vec<PathBuf>> {
    let working_files = working_files().context("Collect filepaths in working tree")?;
    let head_commit = retrieve_head_commit().context("Load HEAD Commit")?;
    let index = Index::load().context("Load index")?;
    Ok(working_files
        .into_iter()
        .filter(|fp| {
            fp.to_str().map(|s| !s.starts_with(".")).unwrap_or(false)
                && !head_commit.tracks(fp)
                && !index.additions.contains_key(fp)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;

    use std::fs;

    #[test]
    fn create_rel_path_from_repo_root() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;

        test_utils::set_dir(&tmpdir, || {
            fs::create_dir(".gitlet")?;
            fs::File::create("t.txt")?;

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
            fs::create_dir(".gitlet")?;
            fs::create_dir("a")?;
            fs::File::create("a/t.txt")?;

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
            fs::create_dir("a")?;
            fs::File::create("a/t.txt")?;

            std::env::set_current_dir("a").context("set current dir to 'tmpdir/a/'")?;
            let fp = std::path::Path::new("t.txt");
            let res = find_working_tree_dir(fp);

            assert!(res.is_err());

            Ok(())
        })
    }

    #[test]
    fn test_is_tracked_by_head() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;
        test_utils::set_dir(&tmpdir, || {
            fs::create_dir_all(".gitlet/commits/9f").context("Create .gitlet/commits/9f")?;
            let mut f =
                fs::File::create(".gitlet/commits/9f/58103e11b63e5ccca06154ab8838be7639a574")
                    .context("Create commit file")?;

            let json = serde_json::json!({
                "hash":"9f58103e11b63e5ccca06154ab8838be7639a574",
                "parent":"",
                "merge_parent":"",
                "message":"first commit",
                "timestamp":1755104961,
                "blobs":{"b.txt":{"hash":"02d92c580d4ede6c80a878bdd9f3142d8f757be8"}}
            });
            serde_json::to_writer(&mut f, &json).context("Write commit json")?;

            let mut head_file = fs::File::create(".gitlet/HEAD").context("Create HEAD file")?;
            head_file.write(b"main")?;

            fs::create_dir(".gitlet/refs").context("Create refs directory")?;
            fs::File::create(".gitlet/refs/main").context("Create main branch ref file")?;

            update_head("9f58103e11b63e5ccca06154ab8838be7639a574")?;

            assert!(is_tracked_by_head(Path::new("b.txt")));

            Ok(())
        })
    }

    #[test]
    fn flat_working_files() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;
        test_utils::set_dir(&tmpdir, || {
            fs::create_dir(".gitlet")?;
            fs::File::create("a.txt")?;
            fs::File::create("b.txt")?;

            let expected: Vec<PathBuf> = ["a.txt", "b.txt"]
                .into_iter()
                .rev()
                .map(|t| std::path::PathBuf::from(t))
                .collect();

            let actual = working_files()?;

            assert_eq!(expected, actual);

            Ok(())
        })
    }

    #[test]
    fn nested_working_files() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;
        test_utils::set_dir(&tmpdir, || {
            let filenames = ["a.txt", "b.txt", "one/c.txt", "one/d.txt", "one/two/e.txt"];

            fs::create_dir(".gitlet")?;
            fs::create_dir_all("one/two")?;
            fs::File::create(".gitletignore")?;
            for f in filenames {
                fs::File::create(f)?;
            }

            let mut expected: Vec<PathBuf> = filenames
                .into_iter()
                .rev()
                .map(|t| std::path::PathBuf::from(t))
                .collect();

            let mut actual = working_files()?;

            assert_eq!(expected.sort(), actual.sort());

            Ok(())
        })
    }

    #[test]
    fn new_branch_has_empty_head_commit_hash() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;
        test_utils::set_dir(&tmpdir, || {
            fs::create_dir_all(".gitlet/refs")?;
            fs::create_dir(".gitlet/commit")?;

            let mut head_file = fs::File::create(".gitlet/HEAD")?;
            head_file.write(b"main")?;

            let mut main_ref = fs::File::create(".gitlet/refs/main")?;
            main_ref.write(b"")?;

            create_branch("test").context("Create 'test' branch")?;

            // Show that it points to the same hash as main/HEAD
            let mut test_ref =
                fs::File::open(".gitlet/refs/test").context("Open 'test' ref file")?;
            let mut test_hash = String::new();
            test_ref
                .read_to_string(&mut test_hash)
                .context("Read 'test' ref file")?;
            assert_eq!("", &test_hash);

            Ok(())
        })
    }

    #[test]
    fn new_branch_has_head_commit_hash() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;
        test_utils::set_dir(&tmpdir, || {
            fs::create_dir_all(".gitlet/refs")?;
            fs::create_dir(".gitlet/commit")?;

            let mut head_file = fs::File::create(".gitlet/HEAD")?;
            head_file.write(b"main")?;

            let mut main_ref = fs::File::create(".gitlet/refs/main")?;
            main_ref.write(b"0452ef28c90d315dc3e05323c18b2e3724f7b275")?;

            create_branch("test").context("Create 'test' branch")?;

            // Show that it points to the same hash as main/HEAD
            let mut test_ref =
                fs::File::open(".gitlet/refs/test").context("Open 'test' ref file")?;
            let mut test_hash = String::new();
            test_ref
                .read_to_string(&mut test_hash)
                .context("Read 'test' ref file")?;
            assert_eq!("0452ef28c90d315dc3e05323c18b2e3724f7b275", &test_hash);

            Ok(())
        })
    }
}
