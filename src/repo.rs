//! This module provides methods for creating a new repository and for interacting with an existing one.

use std::collections::HashMap;
use std::fs::{self, read_dir};
use std::io::{self, Read, Write};
use std::path::{self, Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use walkdir::WalkDir;

use crate::blob::Blob;
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

    let branch_name = get_head_branch()?;
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

/// Displays a list of branches, marking the one currently checked out with an asterisk.
pub fn branch(branch_name: Option<String>, delete: bool) -> Result<()> {
    if delete {
        if let Some(branch_name) = branch_name {
            return delete_branch(&branch_name);
        } else {
            anyhow::bail!("Branch name required");
        }
    } else if let Some(branch_name) = branch_name {
        return create_branch(&branch_name);
    }

    let repo_root = abs_path_to_repo_root().context("Get absolute path to repo directory")?;
    let head_branch: std::ffi::OsString = get_head_branch()
        .context("Get name of currently checked out branch")?
        .into();

    let mut branches: Vec<_> = repo_root
        .join(".gitlet/refs")
        .read_dir()
        .context("Read refs directory")?
        .filter_map(Result::ok) // To skip Err entries
        .filter(|e| e.file_type().is_ok_and(|f| f.is_file())) // Keep only files
        .collect();

    branches.sort_by_key(|e| e.file_name());

    for entry in branches {
        let branch_name = entry.file_name();
        if head_branch == branch_name {
            println!("* {}", branch_name.display());
        } else {
            println!("  {}", branch_name.display());
        }
    }

    Ok(())
}

fn create_branch(branch_name: &str) -> Result<()> {
    // Create the path to the named branch.
    let branch_path = abs_path_to_repo_root()
        .context("Get absolute path to working tree root")?
        .join(".gitlet/refs")
        .join(branch_name);

    if branch_path.exists() {
        anyhow::bail!("A branch named '{branch_name}' already exists");
    }

    let head_hash = read_head_hash().context("Get HEAD commit hash")?;

    let mut f = fs::File::create_new(branch_path)
        .with_context(|| format!("A branch named '{branch_name}' already exists"))?;

    f.write_all(head_hash.as_bytes())
        .context("Write HEAD hash to new branch ref")?;

    Ok(())
}

/// Deletes the named branch.
///
/// # Panics
///
/// Panics if the named branch is currently checked out or does not exist.
fn delete_branch(branch_name: &str) -> Result<()> {
    let current_branch = get_head_branch().context("Get current branch name")?;
    if branch_name == current_branch {
        anyhow::bail!("Cannot delete branch when it is checked out");
    }

    // Create the path to the named branch.
    let branch_path = abs_path_to_repo_root()
        .context("Get absolute path to working tree root")?
        .join(".gitlet/refs")
        .join(branch_name);

    if !branch_path.exists() {
        anyhow::bail!(
            "Branch '{}' not found",
            branch_path.file_name().unwrap().display()
        );
    }

    fs::remove_file(&branch_path).with_context(|| {
        format!(
            "Delete branch '{}'",
            branch_path.file_name().unwrap().display()
        )
    })?;

    println!("Deleted branch '{branch_name}'");

    Ok(())
}

/// Switches to the named branch if it exists. If it does not exist and `create` is set, then it
/// creates the branch and switches to it.
///
/// # Panics
///
/// Returns an error if the named branch does not exist and `create` is not set, or vice versa.
pub fn switch(branch_name: &str, create: bool) -> Result<()> {
    // Is it already checked out?
    let current_branch = get_head_branch().context("Get current branch name")?;
    if branch_name == current_branch {
        println!("Already on '{branch_name}'");
        return Ok(());
    }

    // Create the path to the named branch.
    let branch_path = abs_path_to_repo_root()
        .context("Get absolute path to working tree root")?
        .join(".gitlet/refs")
        .join(branch_name);

    // Does the branch exist?
    if branch_path.exists() {
        return checkout_branch(branch_name);
    }

    // No?
    // Is create true?
    if create {
        // Yes: Create it and checkout.
        create_branch(branch_name).with_context(|| format!("Create branch '{branch_name}'"))?;
        return checkout_branch(branch_name);
    }

    // No: Bail!
    anyhow::bail!("invalid reference: '{branch_name}'")
}

/// Checks out the head commit of the named branch.
fn checkout_branch(branch_name: &str) -> Result<()> {
    let repo_root = abs_path_to_repo_root().context("Get absolute path to repo root")?;

    let branch_ref = std::fs::read_to_string(repo_root.join(".gitlet/refs").join(branch_name))
        .context("Read current HEAD commit")?;
    if !branch_ref.is_empty() && branch_ref.len() != 40 {
        anyhow::bail!("Invalid commit");
    }

    checkout_commit(&branch_ref).with_context(|| format!("Checkout commit {branch_ref}"))?;

    let mut head_file =
        std::fs::File::create(repo_root.join(".gitlet/HEAD")).context("Open HEAD file")?;
    head_file
        .write_all(branch_name.as_bytes())
        .context("Write branch name to HEAD file")?;

    println!("Switched to branch '{branch_name}'");

    Ok(())
}

/// Checks out the given commit.
///
/// # Panics
///
/// Panics when there is a modified tracked file that differs (or does not exist) in the destination
/// commit.
fn checkout_commit(hash: &str) -> Result<()> {
    let src_commit_hash = &read_head_hash().context("Get hash of current HEAD commit")?;

    let src_tracked_files = get_commit_blobs(src_commit_hash)
        .context("Get collection of current HEAD's tracked files")?;
    let dst_tracked_files =
        get_commit_blobs(hash).context("Get collection of current HEAD's tracked files")?;

    // For modified tracked files, bail if the file is tracked by the destination commit
    // but it differs.
    let mut modified_tracked_files: Vec<PathBuf> = Vec::new();
    let mut conflicts: Vec<PathBuf> = Vec::new();

    for filepath in unstaged_modifications().context("Collect paths of unstaged modified files")? {
        // In case it is a deleted file, split it at ' (deleted)' and return the file name.
        let filepath = PathBuf::from(filepath.split_whitespace().next().unwrap());

        if file_differs_between_commits(&filepath, &src_tracked_files, &dst_tracked_files)
            .context("Compare tracked versions of file")?
        {
            conflicts.push(filepath);
        } else {
            modified_tracked_files.push(filepath);
        }
    }

    let index = Index::load().context("Load the staging area")?;
    // Check files staged for addition.
    for filepath in index.additions.keys() {
        if file_differs_between_commits(filepath, &src_tracked_files, &dst_tracked_files)
            .context("Compare tracked versions of file")?
        {
            conflicts.push(filepath.clone());
        } else {
            modified_tracked_files.push(filepath.clone());
        }
    }

    // Check files staged for removal.
    for filepath in index.removals.iter() {
        if file_differs_between_commits(filepath, &src_tracked_files, &dst_tracked_files)
            .context("Compare tracked versions of file")?
        {
            conflicts.push(filepath.clone());
        } else {
            modified_tracked_files.push(filepath.clone());
        }
    }

    // Report files that would be overwritten and then bail.
    if !conflicts.is_empty() {
        eprintln!("Your local changes to the following files would be overwritten by checkout:");
        for f in conflicts {
            eprintln!("\t {}", f.display());
        }
        anyhow::bail!("")
    }

    // Save current working directory.
    let initial_dir = std::env::current_dir().context("Get current working directory")?;
    // Change to root of the repository.
    std::env::set_current_dir(abs_path_to_repo_root().context("Get repository root directory")?)
        .context("Set current working directory to the root of the repository")?;

    // Delete files tracked by current commit and untracked by target commit.
    for filepath in src_tracked_files.keys() {
        if !modified_tracked_files.contains(filepath) && !dst_tracked_files.contains_key(filepath) {
            fs::remove_file(filepath)
                .with_context(|| format!("Delete file '{}'", filepath.display()))?;

            // Subdirectories left empty need to be removed, too, but only those that become
            // empty, since otherwise untracked empty trees would be deleted, too, which we don't
            // want. Need as well to walk up the directory tree until finding a non-empty tree.
            // However, as in Git, when a command is issued while in a subtree that only exists in
            // the commit being switched from, the subtree is kept.
            let dirpath = filepath;
            while let Some(dirpath) = dirpath.parent() {
                // Ensure not at repo root, which is an empty path.
                if read_dir(dirpath)
                    .map(|mut e| e.next().is_none())
                    .unwrap_or(false)
                {
                    // Do not delete if it was the directory from which the command was issued.
                    // For example, initial_dir may be '/var/tmp/repo/sub', and the filepath may be
                    // 'sub/a.xt', which leaves a dirpath of 'sub' ('repo/' being the root dir).
                    let abs_dirpath = fs::canonicalize(dirpath).with_context(|| {
                        format!("Create absolute path for '{}'", dirpath.display())
                    })?;
                    if initial_dir == abs_dirpath {
                        break;
                    }

                    fs::remove_dir(dirpath)
                        .with_context(|| format!("Remove dir '{}'", dirpath.display()))?;
                } else {
                    break;
                }
            }
        }
    }

    // Load file contents from destination commit's blobs, skipping those with staged or
    // unstaged modifications.
    for (filepath, blob) in dst_tracked_files.iter() {
        if !modified_tracked_files.contains(filepath) {
            // No need to restore file if it is the same.
            if let Some(src_blob) = src_tracked_files.get(filepath)
                && src_blob.hash == blob.hash
            {
                continue;
            }
            blob.restore(filepath)?;
        }
    }

    // Revert to initial working directory.
    std::env::set_current_dir(&initial_dir).context("Reset working directory to where it was")?;

    Ok(())
}

fn file_differs_between_commits(
    filepath: &Path,
    src_tracked_files: &HashMap<PathBuf, Blob>,
    dst_tracked_files: &HashMap<PathBuf, Blob>,
) -> Result<bool> {
    match (
        src_tracked_files.get(filepath),
        dst_tracked_files.get(filepath),
    ) {
        (Some(src), Some(dst)) => Ok(src.hash != dst.hash),
        (_, _) => Ok(false),
    }
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
pub(crate) fn find_working_tree_dir(filepath: &Path) -> Result<PathBuf> {
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
fn abs_path_working_file(fp: &Path) -> Result<path::PathBuf> {
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
fn get_head_branch() -> Result<String> {
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

    if !branch_ref.is_empty() && branch_ref.len() != 40 {
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
