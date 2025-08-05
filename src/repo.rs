use std::fs;
use std::path::Path;

/// Represents a gitlet repository. This module provides methods for creating a new repository
/// and for interacting with an existing one.
// pub struct Repo {}

/// Initializes a new gitlet repository. `repo_path` is an optional argument passed to
/// `gitlet init` to specify the directory for the new repository. It defaults to the PWD.
pub fn init(repo_dir: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    // If a repository directory was provided, then convert it to a Path,
    // otherwise, use the PWD.
    let repo_dir = match repo_dir {
        Some(dir) => dir.clone(),
        None => ".".to_string(),
    };
    let rpath = Path::new(&repo_dir);

    if rpath.join(".gitlet").exists() {
        return Err("A gitlet repository already exists in this directory".into());
    }

    if !rpath.exists() {
        fs::create_dir(rpath).expect("Failed to create directory for repository");
    }

    fs::create_dir(rpath.join(".gitlet"))?;
    fs::create_dir(rpath.join(".gitlet/blobs"))?;
    fs::create_dir(rpath.join(".gitlet/commits"))?;
    fs::create_dir(rpath.join(".gitlet/refs"))?;
    fs::create_dir(rpath.join(".gitlet/index"))?;
    fs::File::create(rpath.join(".gitlet/HEAD"))?;

    println!("Initialized empty Gitlet repository");

    Ok(())
}
