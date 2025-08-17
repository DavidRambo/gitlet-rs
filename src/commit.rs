//! Represents a Gitlet commit object, which records a snapshot of the working tree in the form of
//! a mapping of filenames to blobs. In addition to this HashMap, a commit comprises a parent
//! commit (or two, in the case of a merge commit), a message, a timestamp, and an id created by
//! taking the sha1 hash of the message, timestamp, and parent commit(s).
use std::collections::HashMap;
use std::fmt::Display;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::{fs, time};

use anyhow::{Context, Result};
use chrono::DateTime;
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
        let blobs = if !parent.is_empty() {
            get_commit_blobs(&parent)?
        } else {
            HashMap::new()
        };

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

/// Returns a commit's HashMap of <filename, blob> entries.
pub(crate) fn get_commit_blobs(commit_id: &str) -> Result<HashMap<PathBuf, Blob>> {
    let commit = Commit::load(commit_id)
        .with_context(|| format!("Load blobs from commit with hash {commit_id}"))?;
    Ok(commit.blobs)
}

/// Formats the commit's information for the log command.
///
/// ===
/// commit [sha1 hash]
/// Date: [timestamp]
/// [commit message]
/// [newline]
impl Display for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::new();
        buf.push_str("===\n");

        buf.push_str("commit ");
        buf.push_str(&self.hash);

        buf.push_str("\nDate: ");
        let date = DateTime::from_timestamp(self.timestamp as i64, 0).unwrap();
        buf.push_str(&date.to_rfc2822());

        buf.push('\n');
        buf.push_str(&self.message);
        buf.push('\n');

        write!(f, "{buf}")
    }
}

/// Data type for iterating through the commit history for the gitlet log command.
///
/// Each iteration returns the `current_hash` and advances it to whichever commit
/// represented by `parent_hash` and `merge_hash` is more recent (i.e. later in time)
/// to the current hash. If there is no `merge_hash`, then it advances to the parent.
pub(crate) struct CommitIter {
    current_hash: Option<String>,
    parent_hash: Option<String>,
    merge_hash: Option<String>,
}

impl Commit {
    pub fn iter(&self) -> CommitIter {
        let (parent_hash, merge_hash) = get_parent_hashes(&self.hash);

        CommitIter {
            current_hash: Some(self.hash.clone()),
            parent_hash,
            merge_hash,
        }
    }
}

impl Iterator for CommitIter {
    type Item = Commit;

    fn next(&mut self) -> Option<Self::Item> {
        self.current_hash.as_ref()?;

        // NOTE: This does not accommodate when a merge_parent is itself a merge commit.
        let output_hash = match (&self.parent_hash, &self.merge_hash) {
            (None, None) => {
                let output_hash = self.current_hash.clone();
                self.current_hash = None;
                output_hash
            }
            (None, Some(hash)) => {
                let output_hash = self.current_hash.clone();
                self.current_hash = self.merge_hash.clone();
                (self.parent_hash, self.merge_hash) = get_parent_hashes(hash);
                output_hash
            }
            (Some(hash), None) => {
                let output_hash = self.current_hash.clone();
                self.current_hash = self.parent_hash.clone();
                (self.parent_hash, self.merge_hash) = get_parent_hashes(hash);
                output_hash
            }
            (Some(parent), Some(merge)) =>
            // todo!("If hashes differ, determine most recent commit. Set that to current_hash. Replace it with its parent.")
            {
                let output_hash = self.current_hash.clone();

                if parent == merge {
                    // Reached point of divergence in history.
                    self.current_hash = self.parent_hash.clone();
                    (self.parent_hash, self.merge_hash) = get_parent_hashes(parent);
                } else {
                    let recent_hash = more_recent_hash(parent, merge);
                    // Possibilities:
                    // None => Fill in all with None.
                    // parent => current = parent, parent = parent's parent, merge stays
                    // merge => current = merge, parent stays, merge = merge's parent
                    match recent_hash {
                        Some(hash) => {
                            if hash == *parent {
                                self.current_hash = Some(parent.clone());
                                (self.parent_hash, _) = get_parent_hashes(parent);
                            } else {
                                self.current_hash = Some(merge.clone());
                                (self.merge_hash, _) = get_parent_hashes(merge);
                            }
                        }
                        None => {
                            self.current_hash = None;
                            self.parent_hash = None;
                            self.merge_hash = None;
                        }
                    }
                }

                output_hash
            }
        };
        Commit::load(&output_hash.unwrap()).ok()
    }
}

fn get_parent_hashes(hash: &str) -> (Option<String>, Option<String>) {
    if hash.is_empty() {
        return (None, None);
    }

    let Ok(commit) = Commit::load(hash) else {
        return (None, None);
    };

    match (commit.parent.is_empty(), commit.merge_parent.is_empty()) {
        (true, true) => (None, None),
        (true, false) => (Some(commit.merge_parent.clone()), None),
        (false, true) => (Some(commit.parent.clone()), None),
        (false, false) => (
            Some(commit.parent.clone()),
            Some(commit.merge_parent.clone()),
        ),
    }
}

fn more_recent_hash(parent: &str, merge: &str) -> Option<String> {
    let parent = Commit::load(parent);
    let merge = Commit::load(merge);

    match (parent, merge) {
        (Ok(p), Ok(m)) => {
            if p.timestamp > m.timestamp {
                Some(p.hash)
            } else {
                Some(m.hash)
            }
        }
        (Ok(p), Err(_)) => Some(p.hash),
        (Err(_), Ok(m)) => Some(m.hash),
        (Err(_), Err(_)) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::test_utils;

    use super::*;

    #[test]
    fn display_commit() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;
        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir_all(".gitlet/commits/9f").context("Create .gitlet/commits/9f")?;
            let mut f =
                std::fs::File::create(".gitlet/commits/9f/58103e11b63e5ccca06154ab8838be7639a574")
                    .context("Create commit file")?;

            let json = serde_json::json!({
                "hash":"9f58103e11b63e5ccca06154ab8838be7639a574",
                "parent":"",
                "merge_parent":"",
                "message":"first commit",
                "timestamp":1755104961,
                "blobs":{"b.txt":{"hash":"02d92c580d4ede6c80a878bdd9f3142d8f757be8"}}
            });
            serde_json::to_writer(&mut f, &json).context("Write commit json")?;

            let mut f = std::fs::File::create(".gitlet/HEAD").context("Create HEAD file")?;
            f.write_all(b"9f58103e11b63e5ccca06154ab8838be7639a574")?;

            let commit = Commit::load("9f58103e11b63e5ccca06154ab8838be7639a574")
                .context("Load commit to test Display trait")?;

            let mut res = vec![];
            write!(&mut res, "{}", commit)?;

            let expected = b"\
                ===\n\
                commit 9f58103e11b63e5ccca06154ab8838be7639a574\n\
                Date: Wed, 13 Aug 2025 17:09:21 +0000\n\
                first commit\n\
                ";

            assert_eq!(res, expected);

            Ok(())
        })
    }
}
