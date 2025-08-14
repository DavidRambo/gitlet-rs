//! Tests the add command.

use std::error::Error;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::predicate;

#[test]
fn stage_file() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;
    let tmp_path = format!("{}/tmp.txt", tmpdir.to_str().unwrap());

    let mut cmd = Command::new("touch");
    cmd.arg(&tmp_path);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg(&tmp_path);
    cmd.assert().success();

    assert!(std::path::Path::new(&format!("{}/.gitlet/index", tmpdir.to_str().unwrap())).exists());

    Ok(())
}

#[test]
fn stage_nonexistent_file() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("tmp.txt");
    cmd.assert().failure().stderr(predicate::str::contains(
        "Cannot stage file. File does not exist.",
    ));

    Ok(())
}
