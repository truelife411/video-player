// 快速内容指纹：<=128MB 完整 SHA-256；更大文件使用头尾各 64MB。
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub const CHUNK: u64 = 64 * 1024 * 1024;

pub fn compute(path: impl AsRef<Path>) -> io::Result<String> {
    compute_with_chunk(path, CHUNK)
}

pub fn compute_legacy(path: impl AsRef<Path>) -> io::Result<String> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let head = hash_bytes(&mut file, size.min(CHUNK))?;
    let tail = if size > CHUNK * 2 {
        file.seek(SeekFrom::Start(size - CHUNK))?;
        hash_bytes(&mut file, CHUNK)?
    } else {
        String::new()
    };
    Ok(format!("{size}:{head}:{tail}"))
}

fn compute_with_chunk(path: impl AsRef<Path>, chunk: u64) -> io::Result<String> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    if size <= chunk * 2 {
        let full = hash_bytes(&mut file, size)?;
        return Ok(format!("{size}:{full}:"));
    }

    let head = hash_bytes(&mut file, chunk)?;
    file.seek(SeekFrom::Start(size - chunk))?;
    let tail = hash_bytes(&mut file, chunk)?;
    Ok(format!("{size}:{head}:{tail}"))
}

fn hash_bytes(reader: &mut File, count: u64) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut remaining = count;
    while remaining > 0 {
        let wanted = remaining.min(buffer.len() as u64) as usize;
        let read = reader.read(&mut buffer[..wanted])?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "文件在计算指纹时发生变化",
            ));
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(value, "{byte:02x}");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn hashes_entire_small_file() {
        let path = std::env::temp_dir().join(format!("video-player-hash-{}", std::process::id()));
        fs::write(&path, b"abcdefghij").unwrap();
        let actual = compute_with_chunk(&path, 8).unwrap();
        let legacy = {
            let mut file = File::open(&path).unwrap();
            format!("10:{}:", hash_bytes(&mut file, 8).unwrap())
        };
        assert_ne!(actual, legacy);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn samples_head_and_tail_of_large_file() {
        let path =
            std::env::temp_dir().join(format!("video-player-hash-large-{}", std::process::id()));
        fs::write(&path, b"abcdefghijklmnopqrst").unwrap();
        let hash = compute_with_chunk(&path, 8).unwrap();
        assert_eq!(hash.matches(':').count(), 2);
        assert!(!hash.ends_with(':'));
        let _ = fs::remove_file(path);
    }
}
