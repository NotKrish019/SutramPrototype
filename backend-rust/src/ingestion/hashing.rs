//! Cryptographic Hashing Module (Member 2 / Evidence Integrity).
//!
//! Provides incremental, streaming SHA-256 calculation and post-storage
//! disk verification without loading full files into memory.

use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// Incremental streaming hasher for chunk-by-chunk cryptographic digest computation.
#[derive(Default)]
pub struct IncrementalHasher {
    hasher: Sha256,
}

impl IncrementalHasher {
    /// Creates a new, initialized SHA-256 streaming hasher.
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
        }
    }

    /// Feeds a chunk of uploaded bytes into the ongoing hash computation.
    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    /// Finalizes the hash state and returns the lowercase 64-character hexadecimal digest.
    pub fn finalize_hex(self) -> String {
        let result = self.hasher.finalize();
        hex::encode(result)
    }
}

/// Asynchronously streams an existing file from disk to verify that its stored bytes
/// exactly match the expected SHA-256 hexadecimal digest.
pub async fn verify_file_sha256(
    file_path: &Path,
    expected_hex: &str,
) -> Result<bool, std::io::Error> {
    let mut file = File::open(file_path).await?;
    let mut hasher = IncrementalHasher::new();
    let mut buffer = [0u8; 64 * 1024]; // 64 KB buffer

    loop {
        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let computed = hasher.finalize_hex();
    Ok(computed.eq_ignore_ascii_case(expected_hex.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_incremental_hasher_matches_known_digest() {
        let mut hasher = IncrementalHasher::new();
        hasher.update(b"hello ");
        hasher.update(b"world");
        let digest = hasher.finalize_hex();

        // sha256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        assert_eq!(
            digest,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[tokio::test]
    async fn test_verify_file_sha256_success() {
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(b"forensic evidence payload")
            .expect("Failed to write");
        temp_file.flush().expect("Failed to flush");

        let mut hasher = IncrementalHasher::new();
        hasher.update(b"forensic evidence payload");
        let expected = hasher.finalize_hex();

        let is_valid = verify_file_sha256(temp_file.path(), &expected)
            .await
            .expect("Verification IO failed");
        assert!(is_valid);
    }

    #[tokio::test]
    async fn test_verify_file_sha256_detects_mismatch() {
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(b"original content")
            .expect("Failed to write");
        temp_file.flush().expect("Failed to flush");

        let is_valid = verify_file_sha256(
            temp_file.path(),
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .await
        .expect("Verification IO failed");
        assert!(!is_valid);
    }
}
