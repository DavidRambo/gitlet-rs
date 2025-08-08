// Handles the hashing of files.
use std::{
    io::{self, BufRead},
    path,
    str::FromStr,
};

use anyhow::{Context, Result};
use sha1::{Digest, Sha1};

/// Represents a blob, which is the gitlet object for a tracked file.
/// 'id': 40-char String produced by the Sha1 hash
/// 'blobpath': Path to the blob
pub struct Blob {
    hash: String,
}

impl Blob {
    /// Constructs a new Blob from the provided file path. This provides the necessary metadata
    /// with which gitlet may stage a file, commit it, and restore it.
    pub fn new(fpath: &path::Path) -> Result<Self> {
        let mut hasher = Sha1::new();

        let f = std::fs::File::open(fpath)
            .with_context(|| format!("opening file for new blob to hash: '{:?}'", fpath))?;
        let buf = io::BufReader::new(&f);

        for bufline in buf.lines() {
            hasher.update(
                bufline.with_context(|| format!("Could not read buffered file `{:?}`", &fpath))?,
            );
        }

        let hash = hasher.finalize();
        let hash = hex::encode(hash);

        Ok(Self { hash })
    }

    /// Constructs a Blob from an existent blob object's id.
    pub fn retrieve(id: &str) -> Result<Self> {
        todo!()
    }

    /// Writes the blob object file using Zlib compression on the file.
    pub fn write_blob(&self, fpath: &path::Path) -> Result<()> {
        todo!();
    }

    /// Reads the blob object file using Zlib decompression to retrieve the file.
    // TODO: Should it return a file or buffer of the file?
    pub fn read_blob(&self, fpath: &path::Path) -> Result<()> {
        todo!();
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
    fn create_blob_from_file() {
        let tmpfile = assert_fs::NamedTempFile::new("tmp.txt").unwrap();
        tmpfile.write_str("Test text.").unwrap();
        let blob = Blob::new(&tmpfile);

        assert!(blob.is_ok());
        let blob = blob.unwrap();

        assert_eq!(blob.hash, "79277d238f6bf9d31f1b9ff463ab5ba3bb23b105");
    }

    #[test]
    #[ignore]
    fn create_blob_from_blob_object() {
        let tmpfile = assert_fs::NamedTempFile::new("tmp.txt").unwrap();
        tmpfile.write_str("Test text.").unwrap();

        // TODO: I'll come back to this test having implemented the compressed file in the blob object.
    }
}
