// Tests the add command.

use std::error::Error;
use std::process::Command;

use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::predicate;

#[test]
#[ignore]
fn stage_file() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;
    let tmp = assert_fs::NamedTempFile::new("tmp.txt")?;

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg(tmp.path());
    cmd.assert().success();

    assert!(std::path::Path::new(".gitlet/index").exists());

    Ok(())
}

#[test]
#[ignore]
fn stage_nonexistent_file() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;
    let mut cmd = Command::cargo_bin("gitlet")?;

    cmd.current_dir(&tmpdir).arg("add").arg("tmp.txt");
    cmd.assert().success().stdout(predicate::str::contains(
        "'tmp.txt' did not match any files",
    ));

    Ok(())
}

#[test]
#[ignore]
fn stage_changes_to_already_staged_file() {
    todo!()
}
