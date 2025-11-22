//! Tests the merge command.

use assert_cmd::Command;
use assert_fs::TempDir;
use predicates::prelude::predicate;
use std::error::Error;

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

#[test]
fn merge_new_file() -> Result<(), Box<dyn Error>> {
    let tmpdir = setup_merge_tests()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("switch").arg("main").unwrap();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("merge").arg("dev");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Merged dev into main"));

    Ok(())
}
