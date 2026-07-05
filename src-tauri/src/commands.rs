// Tauri 命令：标签 CRUD、视频记录管理、搜索
use crate::{db, hash};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::AppHandle;

// ============ 数据结构 ============

#[derive(Serialize, Deserialize, Debug)]
pub struct VideoInfo {
    pub hash: String,
    pub file_name: String,
    pub file_path: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_at: i64,
    pub play_position: f64,
    pub duration: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TagType {
    pub id: i64,
    pub name: String,
    pub value_type: String, // "enum" | "free"
    pub is_preset: bool,
    pub sort_order: i64,
    pub options: Vec<String>, // 枚举型的候选值；自由型为空
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VideoTag {
    pub type_id: i64,
    pub type_name: String,
    pub value_type: String,
    pub value: String,
}

// ============ 视频 / hash 命令 ============

/// 计算文件 hash（供前端异步调用）
#[tauri::command]
pub fn compute_video_hash(path: String) -> Result<String, String> {
    hash::compute(&path).map_err(|e| e.to_string())
}

/// 注册/更新视频记录（算 hash 时一并存入元信息），返回 hash
#[tauri::command]
pub fn register_video(app: AppHandle, path: String) -> Result<String, String> {
    let h = hash::compute(&path).map_err(|e| e.to_string())?;
    let p = Path::new(&path);
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let modified_at = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let file_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let extension = p
        .extension()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let size_bytes = meta.len() as i64;
    let now = chrono::Utc::now().timestamp_millis();

    let conn = db::open(&app).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO videos (hash, file_name, file_path, extension, size_bytes, modified_at, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(hash) DO UPDATE SET
            file_name=excluded.file_name,
            file_path=excluded.file_path,
            extension=excluded.extension,
            size_bytes=excluded.size_bytes,
            modified_at=excluded.modified_at",
        params![h, file_name, path, extension, size_bytes, modified_at, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(h)
}

/// 获取视频记录（按 hash）
#[tauri::command]
pub fn get_video(app: AppHandle, hash: String) -> Result<Option<VideoInfo>, String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT hash, file_name, file_path, extension, size_bytes, modified_at, play_position, duration FROM videos WHERE hash=?1")
        .map_err(|e| e.to_string())?;
    let v = stmt
        .query_row(params![hash], |r| {
            Ok(VideoInfo {
                hash: r.get(0)?,
                file_name: r.get(1)?,
                file_path: r.get(2)?,
                extension: r.get(3)?,
                size_bytes: r.get(4)?,
                modified_at: r.get(5)?,
                play_position: r.get(6)?,
                duration: r.get(7).unwrap_or(0.0),
            })
        })
        .ok();
    Ok(v)
}

/// 保存播放进度
#[tauri::command]
pub fn save_play_position(
    app: AppHandle,
    hash: String,
    position: f64,
    duration: f64,
) -> Result<(), String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE videos SET play_position=?1, duration=?2 WHERE hash=?3",
        params![position, duration, hash],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ============ 标签类型 / 选项 命令 ============

/// 列出所有标签类型（含枚举候选值）
#[tauri::command]
pub fn list_tag_types(app: AppHandle) -> Result<Vec<TagType>, String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, value_type, is_preset, sort_order FROM tag_types ORDER BY sort_order, id")
        .map_err(|e| e.to_string())?;
    let types: Vec<(i64, String, String, i64, i64)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut result = Vec::new();
    for (id, name, vt, preset, order) in types {
        let options = if vt == "enum" {
            // 用块作用域确保 opt_stmt 在 collect 后才 drop，避免借用纠缠
            load_options(&conn, id)?
        } else {
            vec![]
        };
        result.push(TagType {
            id,
            name,
            value_type: vt,
            is_preset: preset != 0,
            sort_order: order,
            options,
        });
    }
    Ok(result)
}

/// 读取某枚举标签类型的候选值列表
fn load_options(conn: &rusqlite::Connection, type_id: i64) -> Result<Vec<String>, String> {
    let mut opt_stmt = conn
        .prepare("SELECT value FROM tag_options WHERE type_id=?1 ORDER BY sort_order, id")
        .map_err(|e| e.to_string())?;
    let rows = opt_stmt
        .query_map(params![type_id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 新建自定义标签类型
#[tauri::command]
pub fn create_tag_type(
    app: AppHandle,
    name: String,
    value_type: String, // "enum" | "free"
    options: Vec<String>,
) -> Result<i64, String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    let max_order: i64 = conn
        .query_row("SELECT COALESCE(MAX(sort_order),0) FROM tag_types", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO tag_types (name, value_type, is_preset, sort_order) VALUES (?1,?2,0,?3)",
        params![name, value_type, max_order + 1],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    if value_type == "enum" {
        for (i, opt) in options.iter().enumerate() {
            conn.execute(
                "INSERT OR IGNORE INTO tag_options (type_id, value, sort_order) VALUES (?1,?2,?3)",
                params![id, opt, i as i64],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(id)
}

/// 删除标签类型（预设标签不可删除）
#[tauri::command]
pub fn delete_tag_type(app: AppHandle, type_id: i64) -> Result<(), String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    let is_preset: bool = conn
        .query_row(
            "SELECT is_preset FROM tag_types WHERE id=?1",
            params![type_id],
            |r| r.get::<_, i64>(0),
        )
        .map(|v| v != 0)
        .map_err(|e| e.to_string())?;
    if is_preset {
        return Err("预设标签不可删除".into());
    }
    conn.execute("DELETE FROM tag_types WHERE id=?1", params![type_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 确保预设标签存在（幂等，用于修复误删）
#[tauri::command]
pub fn ensure_presets(app: AppHandle) -> Result<(), String> {
    db::ensure_presets(&app).map_err(|e| e.to_string())
}

// ============ 视频标签 命令 ============

/// 列出某视频的所有标签值
#[tauri::command]
pub fn list_video_tags(app: AppHandle, video_hash: String) -> Result<Vec<VideoTag>, String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT vt.type_id, tt.name, tt.value_type, vt.value_text
             FROM video_tags vt
             JOIN tag_types tt ON tt.id = vt.type_id
             WHERE vt.video_hash=?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![video_hash], |r| {
            Ok(VideoTag {
                type_id: r.get(0)?,
                type_name: r.get(1)?,
                value_type: r.get(2)?,
                value: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 设置（upsert）某视频的某标签值；value 传空字符串 = 清除该标签
#[tauri::command]
pub fn set_video_tag(
    app: AppHandle,
    video_hash: String,
    type_id: i64,
    value: String,
) -> Result<(), String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    if value.trim().is_empty() {
        conn.execute(
            "DELETE FROM video_tags WHERE video_hash=?1 AND type_id=?2",
            params![video_hash, type_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO video_tags (video_hash, type_id, value_text) VALUES (?1,?2,?3)
             ON CONFLICT(video_hash, type_id) DO UPDATE SET value_text=excluded.value_text",
            params![video_hash, type_id, value],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============ 搜索 命令 ============

/// 按关键词搜索：匹配 file_name 或任何标签值
#[tauri::command]
pub fn search_videos(app: AppHandle, keyword: String) -> Result<Vec<VideoInfo>, String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    let like = format!("%{}%", keyword.trim());
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT v.hash, v.file_name, v.file_path, v.extension, v.size_bytes, v.modified_at, v.play_position, v.duration
             FROM videos v
             LEFT JOIN video_tags vt ON vt.video_hash = v.hash
             WHERE v.file_name LIKE ?1 OR vt.value_text LIKE ?1
             ORDER BY v.modified_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![like], |r| {
            Ok(VideoInfo {
                hash: r.get(0)?,
                file_name: r.get(1)?,
                file_path: r.get(2)?,
                extension: r.get(3)?,
                size_bytes: r.get(4)?,
                modified_at: r.get(5)?,
                play_position: r.get(6)?,
                duration: r.get(7).unwrap_or(0.0),
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 在资源管理器中显示文件
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    // Windows：explorer.exe /select,"path"
    std::process::Command::new("explorer.exe")
        .args(["/select,", &path])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
