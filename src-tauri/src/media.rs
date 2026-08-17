use std::path::Path;

pub const VIDEO_EXTS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "webm", "flv", "ts", "m4v", "wmv", "mpg", "mpeg", "vob",
];

pub const SUBTITLE_EXTS: &[&str] = &["srt", "ass", "ssa", "vtt", "sub"];

pub fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
}

pub fn is_video_path(path: &Path) -> bool {
    extension(path).is_some_and(|ext| VIDEO_EXTS.contains(&ext.as_str()))
}

pub fn is_subtitle_path(path: &Path) -> bool {
    extension(path).is_some_and(|ext| SUBTITLE_EXTS.contains(&ext.as_str()))
}

pub fn video_file_from_args<I, S>(args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().find_map(|arg| {
        let value = arg.as_ref();
        if value.starts_with('-') {
            return None;
        }
        is_video_path(Path::new(value)).then(|| value.to_string())
    })
}

pub fn validate_video_file(path: &Path) -> Result<(), String> {
    if !is_video_path(path) {
        return Err("不支持的视频文件格式".into());
    }
    let metadata = path
        .metadata()
        .map_err(|e| format!("无法读取视频文件: {e}"))?;
    if !metadata.is_file() {
        return Err("视频路径不是普通文件".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_first_video_argument() {
        let args = ["--flag", "C:/电影/示例.MP4", "later.mkv"];
        assert_eq!(video_file_from_args(args), Some("C:/电影/示例.MP4".into()));
    }

    #[test]
    fn rejects_non_video_suffixes_and_options() {
        assert_eq!(video_file_from_args(["--file=a.mp4", "a.mp4.txt"]), None);
    }
}
