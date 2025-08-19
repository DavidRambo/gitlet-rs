//! Tests the switch commands

use std::error::Error;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::predicate;

#[test]
fn already_on_branch() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("switch").arg("main");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Already on 'main'"));

    Ok(())
}

#[test]
fn cannot_create_branch_that_exists_short() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    // "Create" a new branch called 'test_branch'
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("switch").arg("-c").arg("main");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Already on 'main'"));

    Ok(())
}

#[test]
fn create_and_switch_to_new_branch() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    // "Create" a new branch called 'test_branch'
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch").arg("test_branch");
    cmd.assert().success();

    // Switch to 'test_branch'
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("switch").arg("test_branch");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Switched to branch 'test_branch"));

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("  main\n* test_branch"));

    Ok(())
}

#[test]
fn switch_to_newly_created_branch() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    // "Create" a new branch called 'a_test_branch'
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("switch")
        .arg("-c")
        .arg("a_test_branch");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("branch");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("* a_test_branch\n  main"));

    Ok(())
}
