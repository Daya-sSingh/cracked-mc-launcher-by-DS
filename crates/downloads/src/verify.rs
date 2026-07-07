use std::path::Path;

use sha1::{Digest, Sha1};
use tokio::io::AsyncReadExt;

/// Hashes a file already on disk without loading it into memory all at
/// once — important here since some library/asset files run into the tens
/// of megabytes and we may be checksumming hundreds of them back-to-back.
pub async fn sha1_of_file(path: &Path) -> std::io::Result<String> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex_encode(&hasher.finalize()))
}

pub fn sha1_of_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_of_bytes_matches_known_vector() {
        // SHA-1("abc") is a standard test vector.
        let digest = sha1_of_bytes(b"abc");
        assert_eq!(digest, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[tokio::test]
    async fn sha1_of_file_matches_sha1_of_bytes() {
        let dir = std::env::temp_dir().join(format!("launcher-sha1-test-{}", uuid_like()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("sample.bin");
        tokio::fs::write(&path, b"the quick brown fox")
            .await
            .unwrap();

        let from_file = sha1_of_file(&path).await.unwrap();
        let from_bytes = sha1_of_bytes(b"the quick brown fox");
        assert_eq!(from_file, from_bytes);

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    /// Tiny dependency-free unique-ish suffix so parallel test runs don't
    /// collide on the same temp directory. Not used for anything security
    /// sensitive — just test isolation.
    fn uuid_like() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
