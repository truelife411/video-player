// 视频播放器 - Tauri 后端入口
mod commands;
mod db;
mod hash;

use std::path::{Path, PathBuf};

/// 给定字幕文件路径，在同目录下查找同名的视频文件。
/// 例如 "D:/movies/Inception.srt" → 若存在 "D:/movies/Inception.mkv" 则返回其路径。
#[tauri::command]
fn find_sibling_video(sub_path: &str) -> Option<String> {
    let p = Path::new(sub_path);
    let dir = p.parent()?;
    let stem = p.file_stem()?.to_str()?;
    let video_exts = [
        "mkv", "mp4", "avi", "mov", "webm", "flv", "ts", "m4v", "wmv", "mpg", "mpeg", "vob",
    ];
    for ext in video_exts {
        let candidate: PathBuf = dir.join(format!("{}.{}", stem, ext));
        if candidate.exists() {
            return candidate.to_str().map(|s| s.to_string());
        }
    }
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_libmpv::init())
        .invoke_handler(tauri::generate_handler![
            find_sibling_video,
            commands::compute_video_hash,
            commands::register_video,
            commands::get_video,
            commands::save_play_position,
            commands::list_tag_types,
            commands::create_tag_type,
            commands::delete_tag_type,
            commands::ensure_presets,
            commands::list_video_tags,
            commands::set_video_tag,
            commands::search_videos,
            commands::reveal_in_explorer,
        ])
        .setup(|app| {
            // 初始化数据库（schema + 预置标签）
            db::init(app.handle()).expect("数据库初始化失败");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用时出错");
}
