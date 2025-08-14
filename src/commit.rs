//! Represents a Gitlet commit object, which records a snapshot of the working tree in the form of
//! a mapping of filenames to blobs. In addition to this HashMap, a commit comprises a parent
//! commit (or two, in the case of a merge commit), a message, a timestamp, and an id created by
//! taking the sha1 hash of the message, timestamp, and parent commit(s).
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::{fs, time};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::blob::{self, Blob};
use crate::{index, repo};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Commit {
    pub(crate) hash: String,
    parent: String,
    merge_parent: String, // Empty string, "", when not a merge.
    message: String,
    timestamp: u64,
    blobs: HashMap<PathBuf, Blob>,
}

impl Commit {
    /// Creates a new commit object using the current state of the index.
    pub fn new(
        parent: String,
        merge_parent: Option<String>,
        message: String,
        index: index::Index,
    ) -> Result<Self> {
        // Check in case this is the first commit.
        let mut blobs: HashMap<PathBuf, Blob> = HashMap::new();
        if !parent.is_empty() {
            blobs = get_commit_blobs(&parent)?;
        }

        let mut blobs: HashMap<PathBuf, Blob> = blobs
            .into_iter()
            .filter(|(k, _)| !index.removals.contains(k))
            .collect();

        // blobs.extend(index.additions);
        for (fname, blob) in index.additions.into_iter() {
            blobs
                .entry(fname)
                .and_modify(|b| {
                    *b = blob::Blob {
                        hash: blob.hash.clone(),
                    }
                })
                .or_insert(blob::Blob {
                    hash: blob.hash.clone(),
                });
        }

        let timestamp = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .context("Create timestamp using UNIX_EPOCH")?
            .as_secs();

        let merge_parent = merge_parent.unwrap_or_default();

        let mut hasher = Sha1::new();
        hasher.update(&parent);
        hasher.update(&merge_parent);
        hasher.update(&message);
        hasher.update(timestamp.to_string());
        let hash = hasher.finalize();
        let hash = hex::encode(hash);

        Ok(Commit {
            hash,
            parent,
            merge_parent,
            message,
            timestamp,
            blobs,
        })
    }

    /// Loads the commit object with the given identifying sha1 hash.
    pub(crate) fn load(hash: &str) -> Result<Self> {
        // For before first commit and the HEAD is empty.
        // NOTE: in the Gitlet spec, initializing a new repo creates an empty first commit.
        if hash.is_empty() {
            return Ok(Commit {
                hash: String::default(),
                parent: String::default(),
                merge_parent: String::default(),
                message: String::default(),
                timestamp: 0,
                blobs: HashMap::default(),
            });
        }

        let commit_path = repo::abs_path_to_repo_root()?
            .join(".gitlet/commits")
            .join(&hash[..2])
            .join(&hash[2..]);

        let mut f = fs::File::open(&commit_path).context("Open commit file")?;

        let stat = fs::metadata(&commit_path).context("Stat the saved commit file")?;
        let mut content = String::with_capacity(stat.len() as usize);
        f.read_to_string(&mut content)
            .context("Read commit file content to string")?;
        let commit: Commit =
            serde_json::from_str(&content).context("Deserialize commit file into memory")?;
        Ok(commit)
    }

    /// Writes the commit object to the repository.
    pub(crate) fn save(self) -> Result<()> {
        let commit_path = repo::abs_path_to_repo_root()?
            .join(".gitlet/commits")
            .join(&self.hash[..2])
            .join(&self.hash[2..]);
        fs::create_dir(commit_path.parent().unwrap())
            .context("create .gitlet/commits/##/ subdirectory")?;

        let commitfile = fs::File::create(commit_path).context("Create commit file")?;
        serde_json::to_writer(commitfile, &self).context("Save commit to .gitlet/commits")?;

        Ok(())
    }

    /// Returns true if the commit tracks the given file.
    pub(crate) fn tracks(&self, filepath: &Path) -> bool {
        self.blobs.contains_key(filepath)
    }
}

// TODO: impl Display for log command

/// Returns a commit's HashMap of <filename, blob> entries.
pub(crate) fn get_commit_blobs(commit_id: &str) -> Result<HashMap<PathBuf, Blob>> {
    let commit = Commit::load(commit_id)
        .with_context(|| format!("Load blobs from commit with hash {commit_id}"))?;
    Ok(commit.blobs)
}
