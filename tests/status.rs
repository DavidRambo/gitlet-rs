// Tests the status command.

use std::error::Error;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::predicate;

#[test]
fn empty_status() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("status");
    cmd.assert().success().stdout(predicate::str::contains(
        "=== Staged Files ===\n\n=== Removed Files ===\n",
    ));

    Ok(())
}

#[test]
fn staged_file_status() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let tmp_path = String::from(&format!("{}/tmp.txt", tmpdir.to_str().unwrap()));

    let mut cmd = Command::new("touch");
    cmd.arg(&tmp_path);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg(&tmp_path);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("status");
    cmd.assert().success().stdout(predicate::str::contains(
        "=== Staged Files ===\ntmp.txt\n\n=== Removed Files ===\n",
    ));

    Ok(())
}
