// Handles the hashing of files.
use std::{
    io::{self, BufRead},
    path,
    str::FromStr,
};

use anyhow::{Context, Result};
use sha1::{Digest, Sha1};

/// Represents a blob, which is the gitlet object for a tracked file.
/// NOTE: It may not be necessary to have all this information, as the hash it what is minimally
/// required.
struct Blob {
    id: String,              // 40-char string produced by the Sha1 hash
    blobpath: path::PathBuf, // Path to the blob
    filepath: path::PathBuf, // Path to the hashed file
                             // File size?
}

impl Blob {
    pub fn new(fpath: &path::Path) -> Result<Self> {
        let mut hasher = Sha1::new();

        let f = std::fs::File::open(fpath)?;
        let buf = io::BufReader::new(&f);

        for bufline in buf.lines() {
            hasher.update(
                bufline.with_context(|| format!("Could not read buffered file `{:?}`", &fpath))?,
            );
        }

        let hash = hasher.finalize();
        let id = hex::encode(hash);
        let bpath = path::PathBuf::from_str(&format!("{}/{}", &id[..2], &id[2..]))?;

        Ok(Self {
            id,
            blobpath: bpath,
            filepath: fpath.to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use std::path::Path;

    #[test]
    fn no_file_to_blob() {
        let blob = Blob::new(Path::new("does/not/exist.txt"));
        assert!(blob.is_err());
    }

    #[test]
    fn create_blob() {
        let tmpfile = assert_fs::NamedTempFile::new("tmp.txt").unwrap();
        tmpfile.write_str("Test text.\n").unwrap();
        let blob = Blob::new(&tmpfile);

        assert!(!blob.is_err());
        let blob = blob.unwrap();

        let expected_bpath = Path::new("79/277d238f6bf9d31f1b9ff463ab5ba3bb23b105");

        assert_eq!(blob.id, "79277d238f6bf9d31f1b9ff463ab5ba3bb23b105");
        assert_eq!(blob.blobpath, expected_bpath);
        assert_eq!(blob.filepath, tmpfile.path());
    }
}
