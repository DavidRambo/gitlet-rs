//! Tests the merge command.

use assert_cmd::Command;
use assert_fs::{
    TempDir,
    assert::PathAssert,
    prelude::{FileWriteStr, PathChild},
};
use predicates::prelude::predicate;
use std::{error::Error, io::Read};

/// Setup function for merge tests.
///
/// Initializes a gitlet repository in the following state:
/// main branch contains an empty file called a.txt.
/// dev branch contains an empty file called b.txt.
/// dev is checked out.
///
/// Returns the TempDir as a Result.
fn setup_merge_tests() -> Result<TempDir, Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init").unwrap();

    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg("a.txt").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("a.txt").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("commit")
        .arg("Create a.txt")
        .unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("switch")
        .arg("-c")
        .arg("dev")
        .unwrap();

    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg("b.txt").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("b.txt").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("commit")
        .arg("Create b.txt")
        .unwrap();

    Ok(tmpdir)
}

/// Attempts to merge with unstaged changes made to the a.txt file in main.
#[test]
fn merge_unstaged_changes() -> Result<(), Box<dyn Error>> {
    let tmpdir = setup_merge_tests()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("switch").arg("main").unwrap();

    let atxt_file = tmpdir.child("a.txt");
    atxt_file.write_str("Some text")?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("merge").arg("dev");
    cmd.assert().failure().stderr(predicate::str::contains(
        "There is a file with unstaged changes.",
    ));

    Ok(())
}

/// Attempts to merge with staged, but uncommitted, changes made to the a.txt file in main.
#[test]
fn merge_uncommitted_changes() -> Result<(), Box<dyn Error>> {
    let tmpdir = setup_merge_tests()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("switch").arg("main").unwrap();

    let mut cmd = Command::new("echo");
    cmd.current_dir(&tmpdir)
        .arg("Some text")
        .arg(">>")
        .arg("a.txt")
        .unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("add")
        .arg("a.txt")
        .assert()
        .success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("merge").arg("dev");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("You have uncommited changes."));

    Ok(())
}

#[test]
fn merge_nonexistent_branch() -> Result<(), Box<dyn Error>> {
    let tmpdir = setup_merge_tests()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("merge").arg("not_here");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("That branch does not exist."));

    Ok(())
}

#[test]
fn merge_self() -> Result<(), Box<dyn Error>> {
    let tmpdir = setup_merge_tests()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("merge").arg("dev");
    cmd.assert().failure().stderr(predicate::str::contains(
        "Cannot merge a branch with itself.",
    ));

    Ok(())
}

#[test]
fn merge_new_file() -> Result<(), Box<dyn Error>> {
    let tmpdir = setup_merge_tests()?;

    // Save dev's commit id.
    let dev_ref = tmpdir.child(".gitlet/refs/dev");
    let mut dev_ref = std::fs::File::open(dev_ref)?;
    let mut dev_commit_id = String::with_capacity(41);
    let _ = dev_ref.read_to_string(&mut dev_commit_id)?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("switch").arg("main").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("merge").arg("dev");
    cmd.assert().success().stdout(predicate::str::contains(
        "Current branch is fast-forwarded.",
    ));

    let mut cmd = Command::new("cat");
    cmd.current_dir(&tmpdir).arg(".gitlet/refs/main");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(&dev_commit_id));

    Ok(())
}

#[test]
fn merge_file_change() -> Result<(), Box<dyn Error>> {
    let tmpdir = setup_merge_tests()?;

    let atxt_file = tmpdir.child("a.txt");
    atxt_file.write_str("Some new text")?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("add")
        .arg("a.txt")
        .assert()
        .success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("commit")
        .arg("Added text to a.txt")
        .assert()
        .success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("switch").arg("main").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("merge").arg("dev");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Current branch is fast-forwarded"));

    atxt_file.assert(predicate::str::contains("Some new text"));

    Ok(())
}

/// Merges branches with separate files, common ancestor.
#[test]
fn merge_split_history() -> Result<(), Box<dyn Error>> {
    let tmpdir = setup_merge_tests()?;

    // Write changes into b.txt on branch dev.
    let btxt_file = tmpdir.child("b.txt");
    btxt_file.write_str("Dev text in b")?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("b.txt").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("commit")
        .arg("Wrote to b.txt")
        .unwrap();

    // Checkout main branch and write and commit changes to a.txt
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("switch").arg("main").unwrap();

    let atxt_file = tmpdir.child("a.txt");
    atxt_file.write_str("Main text")?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("a.txt").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("commit")
        .arg("Wrote 'Main text' to a.txt")
        .unwrap();

    // Merge with dev and check that a.txt and b.txt contain correct text.
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("merge").arg("dev");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Merged dev into main"));

    atxt_file.assert(predicate::str::contains("Main text"));
    btxt_file.assert(predicate::str::contains("Dev text in b"));

    Ok(())
}

/// Merges a file with a conflict.
///
/// First adds "Dev text" to a.txt in the dev branch.
/// Then checks out main and modifies a.txt to contain 'Main text'.
/// Adds and commits the change, then merges with dev branch.
/// a.txt should contain:
///     <<<<<<< HEAD
///     Main text
///     =======
///     Dev text
///     >>>>>>> {head_dev_commit_id}
#[test]
fn merge_file_conflict() -> Result<(), Box<dyn Error>> {
    let tmpdir = setup_merge_tests()?;

    let atxt_file = tmpdir.child("a.txt");
    atxt_file.write_str("Dev text")?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("a.txt").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("commit")
        .arg("Wrote 'Dev text' to a.txt")
        .unwrap();

    // Save dev's commit id.
    let head_file = tmpdir.child(".gitlet/refs/dev");
    let mut head_file = std::fs::File::open(head_file)?;
    let mut dev_commit_id = String::with_capacity(41);
    let _ = head_file.read_to_string(&mut dev_commit_id)?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("switch").arg("main").unwrap();

    let atxt_file = tmpdir.child("a.txt");
    atxt_file.write_str("Main text")?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("a.txt").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("commit")
        .arg("Wrote 'Main text' to a.txt")
        .unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("merge").arg("dev");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Encountered a merge conflict."));

    let expected = "<<<<<<< HEAD\nHead text\n=======\nDev text\n>>>>>>> {dev_commit_id}\n";
    atxt_file.assert(predicate::str::contains(expected));

    Ok(())
}

/// Ensures that an attempted merge between branches with disparate commit histories fails.
/// For a repo to end up in this bad state, a branch would need to be manually created
/// in the .gitlet directory or an existing branch's commit history would need to be
/// manually tampered with.
#[test]
fn merge_no_split() -> Result<(), Box<dyn Error>> {
    todo!()
}
