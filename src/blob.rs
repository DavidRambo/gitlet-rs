//! Handles the hashing of files into blob objects, including reading and writing them to the
//! .gitlet/blobs directory.
use std::{
    fs,
    io::{self, BufReader, prelude::*},
    path,
};

use anyhow::{Context, Result};
use flate2::write::ZlibEncoder;
use flate2::{Compression, write::ZlibDecoder};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

/// Represents a blob, which is the gitlet object for a tracked file.
/// 'id': 40-char String produced by the Sha1 hash
/// 'blobpath': Path to the blob
#[derive(Deserialize, Serialize)]
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
    pub fn retrieve(hash: &str) -> Result<Self> {
        let blobpath = path::Path::new(".gitlet/blobs")
            .join(&hash[..2])
            .join(&hash[2..]);

        anyhow::ensure!(blobpath.exists(), "The provided blob object does not exist");

        Ok(Blob {
            hash: hash.to_string(),
        })
    }

    /// Writes the blob object file using Zlib compression on the file.
    pub fn write_blob(&self, fpath: &path::Path) -> Result<()> {
        let blobpath = path::Path::new(".gitlet/blobs")
            .join(&self.hash[..2])
            .join(&self.hash[2..]);
        fs::create_dir_all(blobpath.parent().unwrap())
            .context("create .gitlet/blobs/##/ subdirectory")?;

        let mut blobfile = fs::File::create(blobpath).context("creating blob file")?;
        let mut f = fs::File::open(fpath).context("opening file in working tree to compress")?;

        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        std::io::copy(&mut f, &mut e).with_context(|| "streaming file into encoder")?;
        blobfile
            .write_all(&e.finish().with_context(|| "finish compression")?)
            .with_context(|| "write compressed file to blob object file")?;

        Ok(())
    }

    /// Reads the blob object file using Zlib decompression to retrieve the file.
    pub fn read_blob(&self, fpath: &path::Path) -> Result<()> {
        let blobpath = path::Path::new(".gitlet/blobs")
            .join(&self.hash[..2])
            .join(&self.hash[2..]);

        let mut blobfile =
            fs::File::open(blobpath).with_context(|| "open blob object file for decompression")?;

        let f = fs::File::create(fpath)
            .with_context(|| "create file in working tree for streaming blob object")?;
        let decoder = ZlibDecoder::new(f);
        let mut decoder = BufReader::new(decoder);

        std::io::copy(&mut decoder, &mut blobfile)
            .with_context(|| "decompressing blob object into working tree file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use std::path::Path;

    use crate::test_utils;

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
    fn write_blob_and_create_blob_from_object() -> Result<()> {
        let tmpdir = assert_fs::TempDir::new()?;

        // std::env::set_current_dir(tmpdir.path())?;
        test_utils::set_dir(&tmpdir, || {
            std::fs::create_dir_all(".gitlet/blobs")?;

            let tmpfile = assert_fs::NamedTempFile::new("tmp.txt").unwrap();
            tmpfile.write_str("Test text.").unwrap();
            let blob = Blob::new(&tmpfile)?;

            blob.write_blob(&tmpfile)?;

            let first_hash = blob.hash;

            let blob = Blob::retrieve(&first_hash)?;
            assert_eq!(first_hash, blob.hash);

            Ok(())
        })
    }
}
