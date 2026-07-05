// 文件指纹：文件大小 + 头尾各 64MB 的 SHA256，组合成稳定 ID
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

const CHUNK: u64 = 64 * 1024 * 1024; // 64 MB

/// 计算文件指纹：返回 "size:headSha:tailSha" 形式的字符串
/// - 小文件（≤ 128MB）整体 hash 头尾会重叠，自动退化为整体 hash
pub fn compute(path: &str) -> io::Result<String> {
    let mut f = File::open(path)?;
    let meta = f.metadata()?;
    let size = meta.len();

    // 头部 64MB（或全文件）
    let head_len = std::cmp::min(size, CHUNK);
    let mut head = Sha256::new();
    read_chunk(&mut f, head_len, &mut head)?;

    let tail_sha = if size > CHUNK * 2 {
        // 文件足够大，读尾部 64MB
        f.seek(SeekFrom::Start(size - CHUNK))?;
        let mut tail = Sha256::new();
        read_chunk(&mut f, CHUNK, &mut tail)?;
        hex(&tail.finalize())
    } else {
        // 文件小，尾部与头部重叠，省略
        String::new()
    };

    Ok(format!("{}:{}:{}", size, hex(&head.finalize()), tail_sha))
}

fn read_chunk(f: &mut File, n: u64, hasher: &mut Sha256) -> io::Result<()> {
    let mut buf = [0u8; 65536];
    let mut left = n;
    while left > 0 {
        let want = std::cmp::min(left, buf.len() as u64) as usize;
        let read = f.read(&mut buf[..want])?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
        left -= read as u64;
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}
