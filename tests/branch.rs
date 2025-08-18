//! Tests the branch commands.

use std::error::Error;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::predicate;

#[test]
fn list_branches() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    // "Create" a new branch called 'test_branch'
    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg(".gitlet/refs/test_branch");
    cmd.assert().success();

    // "Create" a new branch called 'a_test_branch'
    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg(".gitlet/refs/a_test_branch");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch");
    cmd.assert().success().stdout(predicate::str::contains(
        "  a_test_branch\n* main\n  test_branch",
    ));

    Ok(())
}

#[test]
fn branch_delete_needs_branch_name() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch").arg("-D");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error: Branch name required"));

    Ok(())
}

#[test]
fn cannot_delete_current_branch() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch").arg("-D").arg("main");
    cmd.assert().failure().stderr(predicate::str::contains(
        "Error: Cannot delete branch when it is checked out",
    ));

    Ok(())
}

#[test]
fn delete_branch_not_found() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch").arg("-D").arg("test");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error: Branch 'test' not found"));

    Ok(())
}

#[test]
fn delete_branch() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    // "Create" a new branch called 'test_branch'
    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg(".gitlet/refs/test_branch");
    cmd.assert().success();

    // "Create" a new branch called 'a_test_branch'
    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg(".gitlet/refs/a_test_branch");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("branch")
        .arg("-D")
        .arg("test_branch");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Deleted branch 'test_branch'"));

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("  a_test_branch\n* main\n"));

    Ok(())
}

#[test]
fn cannot_create_branch_that_exists() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    // "Create" a new branch called 'test_branch'
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch").arg("main");
    cmd.assert().failure().stderr(predicate::str::contains(
        "Error: A branch named 'main' already exists",
    ));

    Ok(())
}

#[test]
fn create_branch_no_commits() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    // "Create" a new branch called 'test_branch'
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch").arg("test_branch");
    cmd.assert().success();

    // "Create" a new branch called 'a_test_branch'
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch").arg("a_test_branch");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch");
    cmd.assert().success().stdout(predicate::str::contains(
        "  a_test_branch\n* main\n  test_branch",
    ));

    Ok(())
}

#[test]
fn create_branch_with_commit() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    // Add and commit new file.
    let tmp_path = format!("{}/tmp.txt", tmpdir.display());
    let mut cmd = Command::new("touch");
    cmd.arg(&tmp_path);
    cmd.assert().success();
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg(&tmp_path);
    cmd.assert().success();
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("commit").arg("add tmp.txt");
    cmd.assert().success();

    // "Create" a new branch called 'test_branch'
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch").arg("test_branch");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("* main\n  test_branch"));

    Ok(())
}
