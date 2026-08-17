use crate::{db, hash, media, probe};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VideoInfo {
    pub hash: String,
    pub file_name: String,
    pub file_path: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_at: i64,
    pub play_position: f64,
    pub duration: f64,
    pub stars: i64,
    pub quality: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TagOption {
    pub id: i64,
    pub value: String,
    pub sort_order: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TagType {
    pub id: i64,
    pub name: String,
    pub value_type: String,
    pub is_preset: bool,
    pub system_key: Option<String>,
    pub is_multi: bool,
    pub sort_order: i64,
    pub options: Vec<TagOption>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VideoTag {
    pub type_id: i64,
    pub type_name: String,
    pub value_type: String,
    pub system_key: Option<String>,
    pub is_multi: bool,
    pub values: Vec<String>,
}

async fn blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| format!("后台任务失败: {e}"))?
}

const VIDEO_SELECT: &str =
    "SELECT v.hash,v.file_name,v.file_path,v.extension,v.size_bytes,v.modified_at,
            v.play_position,v.duration,
            (SELECT CAST(vt.value_text AS INTEGER) FROM video_tags vt
             JOIN tag_types tt ON tt.id=vt.type_id
             WHERE vt.video_hash=v.hash AND tt.system_key='stars'),
            COALESCE((SELECT vt.value_text FROM video_tags vt
             JOIN tag_types tt ON tt.id=vt.type_id
             WHERE vt.video_hash=v.hash AND tt.system_key='quality'),'')
     FROM videos v";

fn video_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VideoInfo> {
    Ok(VideoInfo {
        hash: row.get(0)?,
        file_name: row.get(1)?,
        file_path: row.get(2)?,
        extension: row.get(3)?,
        size_bytes: row.get(4)?,
        modified_at: row.get(5)?,
        play_position: row.get(6)?,
        duration: row.get::<_, Option<f64>>(7)?.unwrap_or(0.0),
        stars: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
        quality: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn compute_video_hash(path: String) -> Result<String, String> {
    blocking(move || {
        let path = PathBuf::from(path);
        media::validate_video_file(&path)?;
        hash::compute(path).map_err(|e| e.to_string())
    })
    .await
}

fn migrate_legacy_hash(conn: &mut Connection, legacy: &str, current: &str) -> Result<(), String> {
    if legacy == current {
        return Ok(());
    }
    let legacy_exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM videos WHERE hash=?1)",
            [legacy],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let current_exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM videos WHERE hash=?1)",
            [current],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !legacy_exists {
        return Ok(());
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if current_exists {
        tx.execute(
            "INSERT INTO video_tags (video_hash,type_id,value_text,sort_order,created_at)
             SELECT ?1,type_id,value_text,sort_order,created_at FROM video_tags WHERE video_hash=?2
             ON CONFLICT(video_hash,type_id,value_text) DO NOTHING",
            params![current, legacy],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE videos SET
                play_position=MAX(play_position,COALESCE((SELECT play_position FROM videos WHERE hash=?2),0)),
                duration=MAX(COALESCE(duration,0),COALESCE((SELECT duration FROM videos WHERE hash=?2),0))
             WHERE hash=?1",
            params![current, legacy],
        )
        .map_err(|e| e.to_string())?;
    } else {
        tx.execute(
            "INSERT INTO videos SELECT ?1,file_name,file_path,extension,size_bytes,modified_at,
                    play_position,duration,created_at FROM videos WHERE hash=?2",
            params![current, legacy],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE video_tags SET video_hash=?1 WHERE video_hash=?2",
            params![current, legacy],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute("DELETE FROM videos WHERE hash=?1", [legacy])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn register_video(app: AppHandle, path: String) -> Result<String, String> {
    blocking(move || {
        let file = PathBuf::from(&path);
        media::validate_video_file(&file)?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        let current_hash = hash::compute(&file).map_err(|e| e.to_string())?;
        let legacy_hash = (metadata.len() > hash::CHUNK && metadata.len() <= hash::CHUNK * 2)
            .then(|| hash::compute_legacy(&file).map_err(|e| e.to_string()))
            .transpose()?;
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        let file_name = file
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or("视频文件名不是有效 Unicode")?
            .to_string();
        let extension = media::extension(&file).ok_or("视频没有扩展名")?;
        let mut conn = db::open(&app)?;
        if let Some(legacy) = legacy_hash {
            migrate_legacy_hash(&mut conn, &legacy, &current_hash)?;
        }
        conn.execute(
            "INSERT INTO videos (hash,file_name,file_path,extension,size_bytes,modified_at,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(hash) DO UPDATE SET file_name=excluded.file_name,file_path=excluded.file_path,
                extension=excluded.extension,size_bytes=excluded.size_bytes,modified_at=excluded.modified_at",
            params![current_hash,file_name,path,extension,metadata.len() as i64,modified_at,
                chrono::Utc::now().timestamp_millis()],
        )
        .map_err(|e| e.to_string())?;
        Ok(current_hash)
    })
    .await
}

#[tauri::command]
pub fn get_video(app: AppHandle, hash: String) -> Result<Option<VideoInfo>, String> {
    db::open(&app)?
        .query_row(
            &format!("{VIDEO_SELECT} WHERE v.hash=?1"),
            [hash],
            video_from_row,
        )
        .optional()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_play_position(
    app: AppHandle,
    hash: String,
    position: f64,
    duration: f64,
) -> Result<(), String> {
    if !position.is_finite() || !duration.is_finite() || position < 0.0 || duration < 0.0 {
        return Err("播放进度必须是有限非负数".into());
    }
    let position = if duration > 0.0 {
        position.min(duration)
    } else {
        position
    };
    db::open(&app)?
        .execute(
            "UPDATE videos SET play_position=?1,duration=?2 WHERE hash=?3",
            params![position, duration, hash],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn load_options(conn: &Connection, type_id: i64) -> Result<Vec<TagOption>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id,value,sort_order FROM tag_options WHERE type_id=?1 ORDER BY sort_order,id",
        )
        .map_err(|e| e.to_string())?;
    let result = statement
        .query_map([type_id], |row| {
            Ok(TagOption {
                id: row.get(0)?,
                value: row.get(1)?,
                sort_order: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string());
    result
}

#[tauri::command]
pub fn list_tag_types(app: AppHandle) -> Result<Vec<TagType>, String> {
    let conn = db::open(&app)?;
    let mut statement = conn
        .prepare(
            "SELECT id,name,value_type,is_preset,system_key,is_multi,sort_order FROM tag_types ORDER BY sort_order,id",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    rows.into_iter()
        .map(
            |(id, name, value_type, preset, system_key, is_multi, sort_order)| {
                Ok(TagType {
                    id,
                    name,
                    options: if value_type == "enum" {
                        load_options(&conn, id)?
                    } else {
                        vec![]
                    },
                    value_type,
                    is_preset: preset != 0,
                    system_key,
                    is_multi: is_multi != 0,
                    sort_order,
                })
            },
        )
        .collect()
}

fn clean_text(value: String, label: &str, max: usize) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > max {
        return Err(format!("{label}长度必须为 1-{max} 个字符"));
    }
    Ok(value)
}

#[tauri::command]
pub fn create_tag_type(
    app: AppHandle,
    name: String,
    value_type: String,
    options: Vec<String>,
    is_multi: Option<bool>,
) -> Result<i64, String> {
    let name = clean_text(name, "标签名称", 64)?;
    if value_type != "enum" && value_type != "free" {
        return Err("标签类型必须是 enum 或 free".into());
    }
    let mut seen = HashSet::new();
    let options = options
        .into_iter()
        .map(|v| clean_text(v, "标签选项", 128))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|v| seen.insert(v.clone()))
        .collect::<Vec<_>>();
    if value_type == "enum" && options.is_empty() {
        return Err("枚举标签至少需要一个选项".into());
    }
    let mut conn = db::open(&app)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let max_order: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(sort_order),0) FROM tag_types",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO tag_types (name,value_type,is_preset,sort_order,is_multi) VALUES (?1,?2,0,?3,?4)",
        params![name, value_type, max_order + 1, is_multi.unwrap_or(false) as i64],
    )
    .map_err(|e| e.to_string())?;
    let id = tx.last_insert_rowid();
    for (index, option) in options.iter().enumerate() {
        tx.execute(
            "INSERT INTO tag_options (type_id,value,sort_order) VALUES (?1,?2,?3)",
            params![id, option, index as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BatchTagRequest {
    pub video_hashes: Vec<String>,
    pub type_id: i64,
    pub operation: String,
    pub values: Vec<String>,
}

#[tauri::command]
pub fn update_tag_type(
    app: AppHandle,
    type_id: i64,
    name: String,
    is_multi: bool,
) -> Result<(), String> {
    let name = clean_text(name, "标签名称", 64)?;
    let conn = db::open(&app)?;
    let system_key: Option<String> = conn
        .query_row(
            "SELECT system_key FROM tag_types WHERE id=?1",
            [type_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if system_key.is_some() && is_multi {
        return Err("系统标签不能改为多值".into());
    }
    if !is_multi {
        let conflicts:i64=conn.query_row("SELECT COUNT(*) FROM (SELECT video_hash FROM video_tags WHERE type_id=?1 GROUP BY video_hash HAVING COUNT(*)>1)",[type_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        if conflicts > 0 {
            return Err(format!("有 {conflicts} 个视频包含多个值，无法改为单值"));
        }
    }
    conn.execute(
        "UPDATE tag_types SET name=?1,is_multi=?2 WHERE id=?3",
        params![name, is_multi as i64, type_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn batch_update_video_tags(app: AppHandle, request: BatchTagRequest) -> Result<i64, String> {
    if request.video_hashes.is_empty() || request.video_hashes.len() > 1000 {
        return Err("批量视频数量必须为 1-1000".into());
    }
    let mut conn = db::open(&app)?;
    let (is_multi, values) = validate_tag_values(&conn, request.type_id, request.values)?;
    if !is_multi && matches!(request.operation.as_str(), "add" | "remove") {
        return Err("单值标签只支持替换或清除".into());
    }
    if !matches!(
        request.operation.as_str(),
        "replace" | "add" | "remove" | "clear"
    ) {
        return Err("未知批量操作".into());
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut updated = 0;
    for hash in request.video_hashes.into_iter().collect::<HashSet<_>>() {
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM videos WHERE hash=?1)",
                [&hash],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !exists {
            continue;
        }
        if matches!(request.operation.as_str(), "replace" | "clear") {
            tx.execute(
                "DELETE FROM video_tags WHERE video_hash=?1 AND type_id=?2",
                params![hash, request.type_id],
            )
            .map_err(|e| e.to_string())?;
        }
        if request.operation == "remove" {
            for value in &values {
                tx.execute(
                    "DELETE FROM video_tags WHERE video_hash=?1 AND type_id=?2 AND value_text=?3",
                    params![hash, request.type_id, value],
                )
                .map_err(|e| e.to_string())?;
            }
        } else if request.operation != "clear" {
            for (index, value) in values.iter().enumerate() {
                tx.execute("INSERT INTO video_tags(video_hash,type_id,value_text,sort_order,created_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT DO NOTHING",params![hash,request.type_id,value,index as i64,chrono::Utc::now().timestamp_millis()]).map_err(|e|e.to_string())?;
            }
        }
        updated += 1;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(updated)
}

#[tauri::command]
pub fn delete_tag_type(app: AppHandle, type_id: i64) -> Result<(), String> {
    if type_id <= 0 {
        return Err("无效的标签类型 ID".into());
    }
    let conn = db::open(&app)?;
    let preset: i64 = conn
        .query_row(
            "SELECT is_preset FROM tag_types WHERE id=?1",
            [type_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if preset != 0 {
        return Err("预设标签不可删除".into());
    }
    conn.execute("DELETE FROM tag_types WHERE id=?1", [type_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn ensure_presets(app: AppHandle) -> Result<(), String> {
    db::ensure_presets(&app)
}

#[tauri::command]
pub fn list_video_tags(app: AppHandle, video_hash: String) -> Result<Vec<VideoTag>, String> {
    let conn = db::open(&app)?;
    let mut statement = conn
        .prepare(
            "SELECT vt.type_id,tt.name,tt.value_type,tt.system_key,tt.is_multi,vt.value_text
         FROM video_tags vt JOIN tag_types tt ON tt.id=vt.type_id
         WHERE vt.video_hash=?1 ORDER BY tt.sort_order,vt.sort_order,vt.value_text",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([video_hash], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let mut grouped: Vec<VideoTag> = Vec::new();
    for (type_id, type_name, value_type, system_key, is_multi, value) in rows {
        if let Some(existing) = grouped.iter_mut().find(|tag| tag.type_id == type_id) {
            existing.values.push(value);
        } else {
            grouped.push(VideoTag {
                type_id,
                type_name,
                value_type,
                system_key,
                is_multi: is_multi != 0,
                values: vec![value],
            });
        }
    }
    Ok(grouped)
}

fn validate_tag_values(
    conn: &Connection,
    type_id: i64,
    values: Vec<String>,
) -> Result<(bool, Vec<String>), String> {
    let (value_type, is_multi, system_key): (String, i64, Option<String>) = conn
        .query_row(
            "SELECT value_type,is_multi,system_key FROM tag_types WHERE id=?1",
            [type_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;
    let mut seen = HashSet::new();
    let values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect::<Vec<_>>();
    let is_multi = is_multi != 0 && system_key.is_none();
    if !is_multi && values.len() > 1 {
        return Err("该标签只能设置一个值".into());
    }
    if values.len() > 100 || values.iter().any(|value| value.chars().count() > 1024) {
        return Err("标签值数量或长度超出限制".into());
    }
    if value_type == "enum" {
        for value in &values {
            let allowed: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM tag_options WHERE type_id=?1 AND value=?2)",
                    params![type_id, value],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if !allowed {
                return Err(format!("标签值“{value}”不在候选项中"));
            }
        }
    }
    Ok((is_multi, values))
}

#[tauri::command]
pub fn set_video_tag_values(
    app: AppHandle,
    video_hash: String,
    type_id: i64,
    values: Vec<String>,
) -> Result<(), String> {
    let mut conn = db::open(&app)?;
    let (_, values) = validate_tag_values(&conn, type_id, values)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM video_tags WHERE video_hash=?1 AND type_id=?2",
        params![video_hash, type_id],
    )
    .map_err(|e| e.to_string())?;
    for (index, value) in values.iter().enumerate() {
        tx.execute(
            "INSERT INTO video_tags(video_hash,type_id,value_text,sort_order,created_at)
             VALUES (?1,?2,?3,?4,?5)",
            params![
                video_hash,
                type_id,
                value,
                index as i64,
                chrono::Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_video_tag_if_absent(
    app: AppHandle,
    video_hash: String,
    type_id: i64,
    value: String,
) -> Result<bool, String> {
    let conn = db::open(&app)?;
    let (_, values) = validate_tag_values(&conn, type_id, vec![value])?;
    let Some(value) = values.into_iter().next() else {
        return Ok(false);
    };
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM video_tags WHERE video_hash=?1 AND type_id=?2)",
            params![video_hash, type_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if exists {
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO video_tags(video_hash,type_id,value_text,sort_order,created_at)
         VALUES (?1,?2,?3,0,?4)",
        params![
            video_hash,
            type_id,
            value,
            chrono::Utc::now().timestamp_millis()
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn set_system_tag_if_absent(
    app: AppHandle,
    video_hash: String,
    system_key: String,
    value: String,
) -> Result<bool, String> {
    if !matches!(system_key.as_str(), "stars" | "quality") {
        return Err("未知系统标签".into());
    }
    let conn = db::open(&app)?;
    db::ensure_presets_on(&conn).map_err(|e| e.to_string())?;
    let type_id: i64 = conn
        .query_row(
            "SELECT id FROM tag_types WHERE system_key=?1",
            [&system_key],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let (_, values) = validate_tag_values(&conn, type_id, vec![value])?;
    let Some(value) = values.into_iter().next() else {
        return Ok(false);
    };
    let changed = conn
        .execute(
            "INSERT INTO video_tags(video_hash,type_id,value_text,sort_order,created_at)
             SELECT ?1,?2,?3,0,?4
             WHERE NOT EXISTS(
               SELECT 1 FROM video_tags WHERE video_hash=?1 AND type_id=?2
             )",
            params![
                video_hash,
                type_id,
                value,
                chrono::Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| e.to_string())?;
    Ok(changed > 0)
}

#[tauri::command]
pub fn set_video_tag(
    app: AppHandle,
    video_hash: String,
    type_id: i64,
    value: String,
) -> Result<(), String> {
    set_video_tag_values(app, video_hash, type_id, vec![value])
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn query_videos(
    conn: &Connection,
    sql: &str,
    value: impl rusqlite::ToSql,
) -> Result<Vec<VideoInfo>, String> {
    let mut statement = conn.prepare(sql).map_err(|e| e.to_string())?;
    let result = statement
        .query_map([value], video_from_row)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string());
    result
}

fn search_videos_in_conn(conn: &Connection, keyword: &str) -> Result<Vec<VideoInfo>, String> {
    let sql = format!("{VIDEO_SELECT} LEFT JOIN video_tags search_tags ON search_tags.video_hash=v.hash WHERE v.file_name LIKE ?1 ESCAPE '\\' OR search_tags.value_text LIKE ?1 ESCAPE '\\' GROUP BY v.hash ORDER BY v.modified_at DESC");
    query_videos(conn, &sql, format!("%{}%", escape_like(keyword)))
}

#[tauri::command]
pub fn search_videos(app: AppHandle, keyword: String) -> Result<Vec<VideoInfo>, String> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(vec![]);
    }
    if keyword.chars().count() > 200 {
        return Err("搜索关键词过长".into());
    }
    let conn = db::open(&app)?;
    search_videos_in_conn(&conn, keyword)
}

#[tauri::command]
pub fn list_videos_by_stars(app: AppHandle, stars: i64) -> Result<Vec<VideoInfo>, String> {
    if !(1..=7).contains(&stars) {
        return Err("星级必须在 1-7 之间".into());
    }
    let conn = db::open(&app)?;
    let sql = format!("{VIDEO_SELECT} WHERE v.hash IN (SELECT vt.video_hash FROM video_tags vt JOIN tag_types tt ON tt.id=vt.type_id WHERE tt.system_key='stars' AND CAST(vt.value_text AS INTEGER)=?1) ORDER BY v.modified_at DESC");
    query_videos(&conn, &sql, stars)
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub keyword: String,
    pub stars: Option<i64>,
    pub sort_key: String,
    pub sort_dir: String,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchPage {
    pub items: Vec<VideoInfo>,
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
    pub total_pages: i64,
}

fn search_videos_page_in_conn(
    conn: &Connection,
    request: SearchRequest,
) -> Result<SearchPage, String> {
    let page_size = request.page_size.clamp(1, 100);
    let mut page = request.page.max(1);
    let keyword = request.keyword.trim();
    if keyword.chars().count() > 200 {
        return Err("搜索关键词过长".into());
    }
    if request.stars.is_some_and(|stars| !(1..=7).contains(&stars)) {
        return Err("星级必须在 1-7 之间".into());
    }
    let like = format!("%{}%", escape_like(keyword));
    let stars = request.stars.unwrap_or(0);
    let where_sql = "WHERE (?1='' OR v.file_name LIKE ?2 ESCAPE '\\' OR EXISTS(SELECT 1 FROM video_tags sx WHERE sx.video_hash=v.hash AND sx.value_text LIKE ?2 ESCAPE '\\'))
      AND (?3=0 OR EXISTS(SELECT 1 FROM video_tags st JOIN tag_types tt ON tt.id=st.type_id WHERE st.video_hash=v.hash AND tt.system_key='stars' AND CAST(st.value_text AS INTEGER)=?3))";
    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM videos v {where_sql}"),
            params![keyword, like, stars],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let total_pages = (total + page_size - 1) / page_size;
    if total_pages > 0 {
        page = page.min(total_pages);
    }
    let sort = match request.sort_key.as_str() {
        "file_name" => "v.file_name",
        "file_path" => "v.file_path",
        "size_bytes" => "v.size_bytes",
        "stars" => "9",
        "quality" => "10",
        _ => "v.modified_at",
    };
    let direction = if request.sort_dir.eq_ignore_ascii_case("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let sql = format!("{VIDEO_SELECT} {where_sql} ORDER BY {sort} {direction} LIMIT ?4 OFFSET ?5");
    let mut statement = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let items = statement
        .query_map(
            params![keyword, like, stars, page_size, (page - 1) * page_size],
            video_from_row,
        )
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(SearchPage {
        items,
        page,
        page_size,
        total,
        total_pages,
    })
}

#[tauri::command]
pub async fn search_videos_page(
    app: AppHandle,
    request: SearchRequest,
) -> Result<SearchPage, String> {
    blocking(move || {
        let conn = db::open(&app)?;
        search_videos_page_in_conn(&conn, request)
    })
    .await
}

#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    if !Path::new(&path).is_file() {
        return Err("文件不存在".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{path}"))
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn validate_setting(key: &str, value: &str) -> Result<(), String> {
    match key {
        "skip_seconds" => {
            let n: u32 = value.parse().map_err(|_| "快进秒数格式无效")?;
            if !(1..=60).contains(&n) {
                return Err("快进秒数必须在 1-60 之间".into());
            }
        }
        "window_scale" => {
            let n: f64 = value.parse().map_err(|_| "窗口缩放格式无效")?;
            if !n.is_finite() || !(0.25..=3.0).contains(&n) {
                return Err("窗口缩放必须在 0.25-3.0 之间".into());
            }
        }
        "resume_mode" if value == "start" || value == "resume" => {}
        "resume_mode" => return Err("播放起点设置无效".into()),
        "window_size_policy" if matches!(value, "video" | "keep" | "fit" | "maximize") => {}
        "window_size_policy" => return Err("窗口尺寸策略无效".into()),
        "volume" => {
            let n: f64 = value.parse().map_err(|_| "音量格式无效")?;
            if !n.is_finite() || !(0.0..=100.0).contains(&n) {
                return Err("音量必须在 0-100 之间".into());
            }
        }
        "muted" if value == "true" || value == "false" => {}
        "muted" => return Err("静音设置无效".into()),
        _ => return Err("未知设置项".into()),
    }
    Ok(())
}

#[tauri::command]
pub fn get_setting(app: AppHandle, key: String) -> Result<Option<String>, String> {
    if !matches!(
        key.as_str(),
        "skip_seconds" | "window_scale" | "resume_mode" | "window_size_policy" | "volume" | "muted"
    ) {
        return Err("未知设置项".into());
    }
    db::open(&app)?
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_setting(app: AppHandle, key: String, value: String) -> Result<(), String> {
    validate_setting(&key, &value)?;
    db::open(&app)?.execute("INSERT INTO settings (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]).map_err(|e|e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn screenshots_dir(app: AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .picture_dir()
        .map_err(|e| format!("无法获取系统图片目录: {e}"))?
        .join("Screenshots");
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建截图目录: {e}"))?;
    dir.to_str()
        .map(str::to_string)
        .ok_or_else(|| "截图目录不是有效 Unicode".into())
}

pub fn probe_resolution(path: &str) -> Option<(u32, u32)> {
    probe::resolution(path)
}

#[tauri::command]
pub async fn probe_video_resolution(path: String) -> Result<Option<(u32, u32)>, String> {
    blocking(move || {
        let path = PathBuf::from(path);
        media::validate_video_file(&path)?;
        Ok(probe::resolution(path))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escapes_like_metacharacters() {
        assert_eq!(escape_like(r"a%_\b"), r"a\%\_\\b");
    }
    #[test]
    fn search_keeps_missing_video_and_tags() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE videos (
                hash TEXT PRIMARY KEY,
                file_name TEXT NOT NULL,
                file_path TEXT NOT NULL,
                extension TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                modified_at INTEGER NOT NULL,
                play_position REAL NOT NULL DEFAULT 0,
                duration REAL,
                created_at INTEGER NOT NULL
             );
             CREATE TABLE tag_types (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                value_type TEXT NOT NULL,
                is_preset INTEGER NOT NULL,
                sort_order INTEGER NOT NULL,
                system_key TEXT,
                is_multi INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE video_tags (
                video_hash TEXT NOT NULL,
                type_id INTEGER NOT NULL,
                value_text TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                PRIMARY KEY(video_hash,type_id,value_text),
                FOREIGN KEY(video_hash) REFERENCES videos(hash) ON DELETE CASCADE
             );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO videos VALUES (?1,'offline.mp4',?2,'mp4',10,1,25,100,1)",
            params!["missing-hash", r"Z:\offline\offline.mp4"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tag_types VALUES (1,'星级','enum',1,0,'stars',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO video_tags VALUES ('missing-hash',1,'5',0,1)",
            [],
        )
        .unwrap();

        let rows = search_videos_in_conn(&conn, "offline").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].stars, 5);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM videos", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM video_tags", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn accepts_camel_case_search_request() {
        let request: SearchRequest = serde_json::from_value(serde_json::json!({
            "keyword": "",
            "stars": 4,
            "sortKey": "modified_at",
            "sortDir": "desc",
            "page": 1,
            "pageSize": 50
        }))
        .unwrap();
        assert_eq!(request.stars, Some(4));
        assert_eq!(request.sort_key, "modified_at");
        assert_eq!(request.page_size, 50);
    }

    #[test]
    fn star_page_filter_returns_matching_video() {
        let mut conn = Connection::open_in_memory().unwrap();
        db::init_connection(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO videos(hash,file_name,file_path,extension,size_bytes,modified_at,play_position,duration,created_at)
             VALUES('four-star','★★★★demo.mp4','D:/★★★★demo.mp4','mp4',10,1,0,100,1)",
            [],
        )
        .unwrap();
        let star_type: i64 = conn
            .query_row(
                "SELECT id FROM tag_types WHERE system_key='stars'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO video_tags(video_hash,type_id,value_text,sort_order,created_at)
             VALUES('four-star',?1,'4',0,1)",
            [star_type],
        )
        .unwrap();

        let request = SearchRequest {
            stars: Some(4),
            sort_key: "modified_at".into(),
            sort_dir: "desc".into(),
            page: 1,
            page_size: 50,
            ..Default::default()
        };
        let result = search_videos_page_in_conn(&conn, request).unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].hash, "four-star");
        assert_eq!(result.items[0].stars, 4);

        let no_match = search_videos_page_in_conn(
            &conn,
            SearchRequest {
                stars: Some(3),
                sort_key: "modified_at".into(),
                sort_dir: "desc".into(),
                page: 1,
                page_size: 50,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(no_match.total, 0);
    }

    #[test]
    fn validates_settings() {
        assert!(validate_setting("skip_seconds", "5").is_ok());
        assert!(validate_setting("skip_seconds", "0").is_err());
        assert!(validate_setting("window_scale", "1.25").is_ok());
        assert!(validate_setting("resume_mode", "other").is_err());
    }
}
