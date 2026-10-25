//! Bounded content fingerprint.
//!
//! Fingerprints exist to *recognise* a file across a rename or a moved drive.
//! They are candidates, not proof: only the size plus the sampled edge bytes are
//! hashed, so two different files with identical edges can collide. Nothing in
//! the core merges two files from a fingerprint alone.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use sha2::{Digest, Sha256};

/// Bytes sampled from the head and tail of a larger file.
const SAMPLE_BYTES: usize = 65_536;

/// Hashes the size plus head/tail samples of a media file.
///
/// Files of `2 * SAMPLE_BYTES` or fewer are hashed in full. The size is mixed in
/// first so a truncated copy cannot share a fingerprint with its original.
pub fn media_fingerprint(path: &Path, size: u64) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    hasher.update(size.to_string().as_bytes());

    let mut head = vec![0_u8; SAMPLE_BYTES];
    let read = read_up_to(&mut file, &mut head)?;
    hasher.update(&head[..read]);

    if size > SAMPLE_BYTES as u64 {
        let offset = (size - SAMPLE_BYTES as u64).clamp(SAMPLE_BYTES as u64, size);
        file.seek(SeekFrom::Start(offset))?;
        let mut tail = vec![0_u8; SAMPLE_BYTES];
        let read = read_up_to(&mut file, &mut tail)?;
        hasher.update(&tail[..read]);
    }

    let digest = hasher.finalize();
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn read_up_to(file: &mut File, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buffer.len() {
        match file.read(&mut buffer[total..])? {
            0 => break,
            read => total += read,
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, bytes: &[u8]) -> u64 {
        let mut file = File::create(path).expect("fixture is writable");
        file.write_all(bytes).expect("fixture write");
        bytes.len() as u64
    }

    #[test]
    fn fingerprint_is_stable_for_identical_bytes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = dir.path().join("a.mkv");
        let second = dir.path().join("b.mkv");
        let size = write(&first, b"same content");
        write(&second, b"same content");
        assert_eq!(
            media_fingerprint(&first, size).unwrap(),
            media_fingerprint(&second, size).unwrap()
        );
    }

    #[test]
    fn fingerprint_changes_with_size() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("a.mkv");
        let size = write(&path, b"content");
        let original = media_fingerprint(&path, size).unwrap();
        assert_ne!(original, media_fingerprint(&path, size + 1).unwrap());
    }

    #[test]
    fn fingerprint_samples_the_tail_of_large_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("a.mkv");
        let mut bytes = vec![0_u8; SAMPLE_BYTES * 3];
        bytes[0] = 1;
        bytes[SAMPLE_BYTES * 2] = 2;
        let size = write(&path, &bytes);
        let original = media_fingerprint(&path, size).unwrap();

        bytes[SAMPLE_BYTES * 3 - 1] = 9;
        let size = write(&path, &bytes);
        assert_ne!(original, media_fingerprint(&path, size).unwrap());

        bytes[SAMPLE_BYTES * 3 - 1] = 0;
        let _ = write(&path, &bytes);
        // Restoring the tail restores the fingerprint: middle bytes are not read.
        assert_eq!(original, media_fingerprint(&path, size).unwrap());
    }

    #[test]
    fn fingerprint_of_empty_file_still_hashes_the_size() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("empty.mkv");
        write(&path, b"");
        let digest = media_fingerprint(&path, 0).unwrap();
        assert_eq!(digest.len(), 64);
    }
}
