use crate::media;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const HEAD_SIZE: usize = 1_000_000;

fn read_head(path: &Path) -> Option<Vec<u8>> {
    let file = File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(HEAD_SIZE);
    file.take(HEAD_SIZE as u64).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn probe_mp4_boxes(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut offset = 0usize;
    while offset.checked_add(8)? <= bytes.len() {
        let size32 = be_u32(bytes, offset)? as u64;
        let kind = bytes.get(offset + 4..offset + 8)?;
        let (header, size) = if size32 == 1 {
            let raw: [u8; 8] = bytes.get(offset + 8..offset + 16)?.try_into().ok()?;
            (16usize, u64::from_be_bytes(raw))
        } else if size32 == 0 {
            (8usize, (bytes.len() - offset) as u64)
        } else {
            (8usize, size32)
        };
        if size < header as u64 {
            return None;
        }
        let end = offset.checked_add(usize::try_from(size).ok()?)?;
        if end > bytes.len() {
            return None;
        }
        let body = offset.checked_add(header)?;
        if kind == b"tkhd" {
            let version = *bytes.get(body)?;
            let width_offset = body.checked_add(if version == 1 { 84 } else { 76 })?;
            let width = be_u32(bytes, width_offset)? >> 16;
            let height = be_u32(bytes, width_offset.checked_add(4)?)? >> 16;
            if let Some(size) = valid_size(width, height) {
                return Some(size);
            }
        } else if matches!(
            kind,
            b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" | b"edts"
        ) {
            if let Some(size) = probe_mp4_boxes(&bytes[body..end]) {
                return Some(size);
            }
        }
        offset = end;
    }
    None
}

fn probe_mp4(bytes: &[u8]) -> Option<(u32, u32)> {
    probe_mp4_boxes(bytes)
}

fn ebml_id(bytes: &[u8], offset: usize) -> Option<(u32, usize)> {
    let first = *bytes.get(offset)?;
    let length = (1..=4).find(|length| first & (0x80 >> (length - 1)) != 0)?;
    let mut value = 0u32;
    for byte in bytes.get(offset..offset.checked_add(length)?)? {
        value = (value << 8) | u32::from(*byte);
    }
    Some((value, length))
}

fn ebml_size(bytes: &[u8], offset: usize) -> Option<(Option<usize>, usize)> {
    let first = *bytes.get(offset)?;
    let length = first.leading_zeros() as usize + 1;
    if length > 8 {
        return None;
    }
    let slice = bytes.get(offset..offset.checked_add(length)?)?;
    let mut value = u64::from(first & (0xff >> length));
    for byte in &slice[1..] {
        value = (value << 8) | u64::from(*byte);
    }
    let unknown = value == (1u64.checked_shl((7 * length) as u32)? - 1);
    Some((
        (!unknown).then(|| usize::try_from(value).ok()).flatten(),
        length,
    ))
}

fn ebml_uint(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || bytes.len() > 4 {
        return None;
    }
    let mut value = 0u32;
    for byte in bytes {
        value = (value << 8) | u32::from(*byte);
    }
    Some(value)
}

fn find_ebml_dimensions(bytes: &[u8], depth: usize) -> Option<(u32, u32)> {
    if depth > 8 {
        return None;
    }
    let mut offset = 0usize;
    let mut width = None;
    let mut height = None;
    while offset < bytes.len() {
        let (id, id_len) = ebml_id(bytes, offset)?;
        let size_offset = offset.checked_add(id_len)?;
        let (size, size_len) = ebml_size(bytes, size_offset)?;
        let body = size_offset.checked_add(size_len)?;
        let end = size
            .and_then(|size| body.checked_add(size))
            .unwrap_or(bytes.len());
        if end > bytes.len() || body > end {
            return None;
        }
        let content = &bytes[body..end];
        match id {
            0xB0 => width = ebml_uint(content),
            0xBA => height = ebml_uint(content),
            0x18538067 | 0x1654AE6B | 0xAE | 0xE0 => {
                if let Some(size) = find_ebml_dimensions(content, depth + 1) {
                    return Some(size);
                }
            }
            _ => {}
        }
        if let (Some(width), Some(height)) = (width, height) {
            return valid_size(width, height);
        }
        offset = end;
    }
    None
}

fn probe_avi(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.get(0..4)? != b"RIFF" || bytes.get(8..12)? != b"AVI " {
        return None;
    }
    let mut offset = 12usize;
    while offset.checked_add(8)? <= bytes.len() {
        let id = bytes.get(offset..offset + 4)?;
        let size = le_u32(bytes, offset + 4)? as usize;
        let body = offset.checked_add(8)?;
        let end = body.checked_add(size)?;
        if end > bytes.len() {
            return None;
        }
        if id == b"avih" && size >= 40 {
            return valid_size(le_u32(bytes, body + 32)?, le_u32(bytes, body + 36)?);
        }
        offset = end.checked_add(size % 2)?;
    }
    None
}

fn valid_size(width: u32, height: u32) -> Option<(u32, u32)> {
    (width > 0 && height > 0 && width <= 100_000 && height <= 100_000).then_some((width, height))
}

pub fn resolution(path: impl AsRef<Path>) -> Option<(u32, u32)> {
    let path = path.as_ref();
    if !media::is_video_path(path) {
        return None;
    }
    let bytes = read_head(path)?;
    match media::extension(path)?.as_str() {
        "mp4" | "m4v" | "mov" => probe_mp4(&bytes),
        "mkv" | "webm" => find_ebml_dimensions(&bytes, 0),
        "avi" => probe_avi(&bytes),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_inputs_do_not_panic() {
        for size in 0..128 {
            let bytes = vec![0xff; size];
            assert!(std::panic::catch_unwind(|| {
                let _ = probe_mp4(&bytes);
                let _ = find_ebml_dimensions(&bytes, 0);
                let _ = probe_avi(&bytes);
            })
            .is_ok());
        }
    }

    #[test]
    fn reads_valid_avi_header() {
        let mut bytes = vec![0u8; 68];
        bytes[0..4].copy_from_slice(b"RIFF");
        bytes[4..8].copy_from_slice(&60u32.to_le_bytes());
        bytes[8..12].copy_from_slice(b"AVI ");
        bytes[12..16].copy_from_slice(b"avih");
        bytes[16..20].copy_from_slice(&48u32.to_le_bytes());
        bytes[52..56].copy_from_slice(&1920u32.to_le_bytes());
        bytes[56..60].copy_from_slice(&1080u32.to_le_bytes());
        assert_eq!(probe_avi(&bytes), Some((1920, 1080)));
    }
}
