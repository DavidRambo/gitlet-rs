// Tests the init command.
use assert_cmd::prelude::*; // Add methods on commands
use assert_fs::prelude::*;
// Add methods on commands
use predicates::prelude::*; // For writing assertions
use std::{error::Error, process::Command}; // Run programs

#[test]
fn init_new_repo_in_cd() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;
    // std::env::set_current_dir(tmpdir.path())?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success().stdout(predicate::str::contains(
        "Initialized empty Gitlet repository",
    ));

    tmpdir.child(".gitlet").assert(predicate::path::is_dir());
    tmpdir
        .child(".gitlet/blobs")
        .assert(predicate::path::is_dir());
    tmpdir
        .child(".gitlet/commits")
        .assert(predicate::path::is_dir());
    tmpdir
        .child(".gitlet/refs")
        .assert(predicate::path::is_dir());
    // tmpdir
    //     .child(".gitlet/index")
    //     .assert(predicate::path::is_dir());
    tmpdir
        .child(".gitlet/HEAD")
        .assert(predicate::path::exists());

    Ok(())
}

#[test]
fn init_new_repo_in_path() -> Result<(), Box<dyn Error>> {
    let mut cmd = Command::cargo_bin("gitlet")?;
    let tmpdir = assert_fs::TempDir::new()?;

    cmd.arg("init").arg(tmpdir.path());
    cmd.assert().success().stdout(predicate::str::contains(
        "Initialized empty Gitlet repository",
    ));

    tmpdir.child(".gitlet").assert(predicate::path::is_dir());
    tmpdir
        .child(".gitlet/blobs")
        .assert(predicate::path::is_dir());
    tmpdir
        .child(".gitlet/commits")
        .assert(predicate::path::is_dir());
    tmpdir
        .child(".gitlet/refs")
        .assert(predicate::path::is_dir());
    // tmpdir
    //     .child(".gitlet/index")
    //     .assert(predicate::path::is_dir());
    tmpdir
        .child(".gitlet/HEAD")
        .assert(predicate::path::exists());

    Ok(())
}

#[test]
fn create_new_repo_dir_and_init() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;
    // std::env::set_current_dir(tmpdir.path())?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init").arg("new_tmp");

    cmd.assert().success().stdout(predicate::str::contains(
        "Initialized empty Gitlet repository",
    ));

    tmpdir
        .child("new_tmp/.gitlet")
        .assert(predicate::path::is_dir());
    tmpdir
        .child("new_tmp")
        .child(".gitlet/blobs")
        .assert(predicate::path::is_dir());
    tmpdir
        .child("new_tmp")
        .child(".gitlet/commits")
        .assert(predicate::path::is_dir());
    tmpdir
        .child("new_tmp")
        .child(".gitlet/refs")
        .assert(predicate::path::is_dir());
    // tmpdir
    //     .child("new_tmp")
    //     .child(".gitlet/index")
    //     .assert(predicate::path::is_dir());
    tmpdir
        .child("new_tmp")
        .child(".gitlet/HEAD")
        .assert(predicate::path::exists());

    Ok(())
}

#[test]
fn init_fails_repo_already_exists() -> Result<(), Box<dyn Error>> {
    let mut cmd = Command::cargo_bin("gitlet")?;
    let tmpdir = assert_fs::TempDir::new()?;
    let tdpath = tmpdir.path();
    std::fs::create_dir(tdpath.join(".gitlet"))
        .expect("Failed to create .gitlet directory in TempDir");

    cmd.arg("init").arg(tdpath);
    cmd.assert().failure().stderr(predicate::str::contains(
        "A gitlet repository already exists in this directory",
    ));

    Ok(())
}
