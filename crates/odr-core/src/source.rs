//! Read-only access to evidence files (ADR-0002).
//!
//! The only way this crate opens a file is [`Evidence::open`], which uses
//! `OpenOptions::new().read(true)` — no write, append, truncate or create.
//! On Windows the file is opened with `FILE_SHARE_READ` only, so another
//! process cannot modify it while we read. SHA-256 is computed on open and can
//! be re-verified with [`Evidence::verify_unchanged`].

use crate::error::OdrError;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// A read-only evidence file with its hash.
#[derive(Debug)]
pub struct Evidence {
    path: PathBuf,
    file: File,
    len: u64,
    sha256: [u8; 32],
}

impl Evidence {
    /// Open read-only and hash the whole file.
    ///
    /// # Errors
    /// I/O errors from opening or reading.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, OdrError> {
        let path = path.as_ref().to_path_buf();
        let mut opts = OpenOptions::new();
        opts.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_SHARE_READ: u32 = 0x0000_0001;
            opts.share_mode(FILE_SHARE_READ);
        }
        let mut file = opts.open(&path)?;
        let len = file.metadata()?.len();
        let sha256 = hash_file(&mut file)?;
        Ok(Self {
            path,
            file,
            len,
            sha256,
        })
    }

    /// File path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// File length in bytes.
    pub const fn len(&self) -> u64 {
        self.len
    }

    /// Whether the file is empty.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// SHA-256 computed at open time.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    /// Read exactly `buf.len()` bytes at `offset` (or fewer at EOF; returns count).
    ///
    /// # Errors
    /// I/O errors.
    pub fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, OdrError> {
        self.file.seek(SeekFrom::Start(offset))?;
        let mut n = 0;
        while let Some(rest) = buf.get_mut(n..) {
            if rest.is_empty() {
                break;
            }
            match self.file.read(rest)? {
                0 => break,
                k => n = n.saturating_add(k),
            }
        }
        Ok(n)
    }

    /// Re-hash and compare with the hash taken at open (REQ-001.1).
    ///
    /// # Errors
    /// I/O errors.
    pub fn verify_unchanged(&mut self) -> Result<bool, OdrError> {
        Ok(hash_file(&mut self.file)? == self.sha256)
    }
}

fn hash_file(file: &mut File) -> Result<[u8; 32], OdrError> {
    file.seek(SeekFrom::Start(0))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        let Some(chunk) = buf.get(..n) else { break };
        if n == 0 {
            break;
        }
        h.update(chunk);
    }
    Ok(h.finalize().into())
}
