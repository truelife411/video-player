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
    pub stars: i64,     // 星级 0-7（0=未标注），来自 video_tags 的「星级」类型
    pub quality: String, // 画质（如 1080p/4K），来自 video_tags 的「画质」类型
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
        .prepare(
            "SELECT v.hash, v.file_name, v.file_path, v.extension, v.size_bytes, v.modified_at, v.play_position, v.duration,
                    (SELECT CAST(vt.value_text AS INTEGER) FROM video_tags vt
                     JOIN tag_types tt ON tt.id = vt.type_id
                     WHERE vt.video_hash = v.hash AND tt.name = '星级') AS stars,
                    COALESCE((SELECT vt.value_text FROM video_tags vt
                     JOIN tag_types tt ON tt.id = vt.type_id
                     WHERE vt.video_hash = v.hash AND tt.name = '画质'), '') AS quality
             FROM videos v WHERE v.hash=?1",
        )
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
                stars: r.get::<_, Option<i64>>(8).ok().flatten().unwrap_or(0),
                quality: r.get::<_, Option<String>>(9).ok().flatten().unwrap_or_default(),
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

/// 按关键词搜索：匹配 file_name 或任何标签值。
/// 同时带出星级、画质（通过子查询从 video_tags 取）。
#[tauri::command]
pub fn search_videos(app: AppHandle, keyword: String) -> Result<Vec<VideoInfo>, String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    let like = format!("%{}%", keyword.trim());
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT v.hash, v.file_name, v.file_path, v.extension, v.size_bytes, v.modified_at, v.play_position, v.duration,
                    (SELECT CAST(vt.value_text AS INTEGER) FROM video_tags vt
                     JOIN tag_types tt ON tt.id = vt.type_id
                     WHERE vt.video_hash = v.hash AND tt.name = '星级') AS stars,
                    COALESCE((SELECT vt.value_text FROM video_tags vt
                     JOIN tag_types tt ON tt.id = vt.type_id
                     WHERE vt.video_hash = v.hash AND tt.name = '画质'), '') AS quality
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
                stars: r.get::<_, Option<i64>>(8).ok().flatten().unwrap_or(0),
                quality: r.get::<_, Option<String>>(9).ok().flatten().unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 按星级筛选视频（纯星级，无关键词）。返回该星级的所有视频，按修改时间倒序。
#[tauri::command]
pub fn list_videos_by_stars(app: AppHandle, stars: i64) -> Result<Vec<VideoInfo>, String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT v.hash, v.file_name, v.file_path, v.extension, v.size_bytes, v.modified_at, v.play_position, v.duration,
                    (SELECT CAST(vt.value_text AS INTEGER) FROM video_tags vt
                     JOIN tag_types tt ON tt.id = vt.type_id
                     WHERE vt.video_hash = v.hash AND tt.name = '星级') AS stars,
                    COALESCE((SELECT vt.value_text FROM video_tags vt
                     JOIN tag_types tt ON tt.id = vt.type_id
                     WHERE vt.video_hash = v.hash AND tt.name = '画质'), '') AS quality
             FROM videos v
             WHERE v.hash IN (
                 SELECT vt.video_hash FROM video_tags vt
                 JOIN tag_types tt ON tt.id = vt.type_id
                 WHERE tt.name = '星级' AND CAST(vt.value_text AS INTEGER) = ?1
             )
             ORDER BY v.modified_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![stars], |r| {
            Ok(VideoInfo {
                hash: r.get(0)?,
                file_name: r.get(1)?,
                file_path: r.get(2)?,
                extension: r.get(3)?,
                size_bytes: r.get(4)?,
                modified_at: r.get(5)?,
                play_position: r.get(6)?,
                duration: r.get(7).unwrap_or(0.0),
                stars: r.get::<_, Option<i64>>(8).ok().flatten().unwrap_or(0),
                quality: r.get::<_, Option<String>>(9).ok().flatten().unwrap_or_default(),
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

// ============ 设置（key-value 持久化） ============

/// 读取一个设置项，不存在则返回 None
#[tauri::command]
pub fn get_setting(app: AppHandle, key: String) -> Result<Option<String>, String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT value FROM settings WHERE key=?1")
        .map_err(|e| e.to_string())?;
    let v = stmt
        .query_row(params![&key], |r| r.get::<_, String>(0))
        .ok();
    Ok(v)
}

/// 写入一个设置项（覆盖）
#[tauri::command]
pub fn set_setting(app: AppHandle, key: String, value: String) -> Result<(), String> {
    let conn = db::open(&app).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![&key, &value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ============ 视频分辨率预解析（消除打开时的窗口跳变）============
//
// 目的：在 loadfile 之前就读出视频分辨率，让前端据此提前算好窗口尺寸，
// 避免出现「默认 1280×720 → 跳到目标尺寸」的跳变。
//
// 只手写最常见的容器头解析（MP4 系 / MKV 系 / AVI），读前 ~1MB 即可。
// 不支持的格式或解析失败统一返回 None，前端会回退到「隐藏窗口→等 mpv→显示」兜底。

/// 读取文件前 N 字节到内存（够解析容器头即可）。
fn read_head(path: &str, n: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity(n);
    f.take(n as u64)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// 大端读 u16/u32。
fn be_u16(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn be_u32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *b.get(o)?,
        *b.get(o + 1)?,
        *b.get(o + 2)?,
        *b.get(o + 3)?,
    ]))
}
fn le_u32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(o)?,
        *b.get(o + 1)?,
        *b.get(o + 2)?,
        *b.get(o + 3)?,
    ]))
}

/// MP4/M4V/MOV：递归查找 moov→trak→mdia→mdhd/stbl，定位 tkhd 里的 width/height（16.16 定点）。
/// mp4 box 层级：moov → trak → mdia → minf → stbl → stsd（含编解码与尺寸），
/// 但更稳的是读 trak 下的 tkhd（track header），它直接含 16.16 定点的宽高。
fn probe_mp4(b: &[u8]) -> Option<(u32, u32)> {
    // 递归遍历 box，在子树里找第一个类型为 tkhd 的 box
    fn find_box<'a>(b: &'a [u8], target: &[u8; 4]) -> Option<usize> {
        let mut o = 0usize;
        while o + 8 <= b.len() {
            let size = be_u32(b, o)? as usize;
            let kind = [b[o + 4], b[o + 5], b[o + 6], b[o + 7]];
            let header = 8usize;
            // size==1 → 64 位 size；size==0 → 到文件尾
            if size == 0 {
                break;
            }
            let big_size = if size == 1 {
                // 64 位 size 占随后 8 字节
                if o + 16 > b.len() {
                    break;
                }
                let hi = be_u32(b, o + 8)? as u64;
                let lo = be_u32(b, o + 12)? as u64;
                ((hi << 32) | lo) as usize
            } else {
                size
            };
            let body_start = if size == 1 { o + 16 } else { o + header };
            let body_end = (o + big_size).min(b.len());

            if kind == *target {
                return Some(body_start);
            }
            // 容器类型：继续往下钻（只对已知容器递归，避免误入纯数据 box）
            if matches!(
                &kind,
                b"moov" | b"trak" | b"mdia" | b"minf" | b"udta" | b"edts" | b"stbl"
            ) {
                if let Some(found) = find_box(&b[body_start..body_end], target) {
                    return Some(body_start + found);
                }
            }
            o += big_size.max(header);
        }
        None
    }

    // 注意：mdat 是巨大的纯数据块，必须跳过；上面递归只进容器 box，不会进 mdat。
    let tkhd_off = find_box(b, b"tkhd")?;
    // tkhd 结构：version(1) + flags(3) + ...
    // version==0: creation(4)+mod(4)+trackID(4)+reserved(4)+dur(4)+reserved(8)+
    //             layer(2)+altGroup(2)+vol(2)+reserved(2)+matrix(36)+width(4)+height(4)
    // version==1: 时间字段是 8 字节
    let ver = *b.get(tkhd_off)?;
    let (w_off, h_off) = if ver == 1 {
        // 1 + 3(flags) + 8 + 8 + 4 + 4 + 8 + 2 + 2 + 2 + 2 + 36 = 84 → width 在 84
        (tkhd_off + 84, tkhd_off + 88)
    } else {
        // 1 + 3(flags) + 4 + 4 + 4 + 4 + 4 + 8 + 2 + 2 + 2 + 2 + 36 = 76 → width 在 76
        (tkhd_off + 76, tkhd_off + 80)
    };
    // width/height 是 16.16 定点（高 16 位是整数部分）
    let w = be_u32(b, w_off)? >> 16;
    let h = be_u32(b, h_off)? >> 16;
    if w > 0 && h > 0 {
        Some((w, h))
    } else {
        None
    }
}

/// MKV/WebM：EBML 解析。定位 Segment → Tracks → 找到带 PixelWidth/PixelHeight 的 Video 元素。
fn probe_mkv(b: &[u8]) -> Option<(u32, u32)> {
    // EBML element id 读法：首字节前导 1 的个数决定 id 长度
    fn read_id(b: &[u8], o: usize) -> Option<(u32, usize)> {
        let first = *b.get(o)?;
        let len = if first & 0x80 != 0 {
            1
        } else if first & 0x40 != 0 {
            2
        } else if first & 0x20 != 0 {
            3
        } else if first & 0x10 != 0 {
            4
        } else {
            return None;
        };
        if o + len > b.len() {
            return None;
        }
        let mut id: u32 = 0;
        for i in 0..len {
            id = (id << 8) | b[o + i] as u32;
        }
        Some((id, len))
    }

    // EBML size：VINT（可变长度整数），首字节前导 0 个数决定长度
    fn read_vint(b: &[u8], o: usize) -> Option<(u64, usize)> {
        let first = *b.get(o)?;
        if first == 0 {
            return None; // 不允许全 0
        }
        let len = first.leading_zeros() as usize + 1;
        if len > 8 || o + len > b.len() {
            return None;
        }
        let mut val: u64 = (first & (0xFF >> len)) as u64;
        for i in 1..len {
            val = (val << 8) | b[o + i] as u64;
        }
        // size==全1（unknown/未知长度）→ 用剩余空间占位
        let all_ones = len == 1 && first == 0xFF;
        let _ = all_ones;
        Some((val, len))
    }

    // 关心的 element id
    // 0x18538067 = Segment, 0x1654AE6B = Tracks, 0xAE = TrackEntry,
    // 0xE0 = Video, 0xB0 = PixelWidth(u16), 0xBA = PixelHeight(u16)
    const ID_SEGMENT: u32 = 0x18538067;
    const ID_TRACKS: u32 = 0x1654AE6B;
    const ID_VIDEO: u32 = 0xE0;
    const ID_PIXEL_WIDTH: u32 = 0xB0;
    const ID_PIXEL_HEIGHT: u32 = 0xBA;

    let mut o = 0usize;
    while o < b.len() {
        let (id, id_len) = match read_id(b, o) {
            Some(v) => v,
            None => break,
        };
        let size_off = o + id_len;
        let (size, size_len) = match read_vint(b, size_off) {
            Some(v) => v,
            None => break,
        };
        let body = size_off + size_len;
        let next = if size == 0 || size as usize > b.len() {
            b.len()
        } else {
            body + size as usize
        };

        if id == ID_SEGMENT {
            // 在 Segment 内找 Tracks
            let tracks_body = {
                let seg = &b[body..next.min(b.len())];
                let mut so = 0usize;
                let mut found: Option<(usize, usize)> = None;
                while so < seg.len() {
                    let (sid, sl) = match read_id(seg, so) {
                        Some(v) => v,
                        None => break,
                    };
                    let (ss, ssl) = match read_vint(seg, so + sl) {
                        Some(v) => v,
                        None => break,
                    };
                    let sb = so + sl + ssl;
                    let sn = if ss == 0 || sb + ss as usize > seg.len() {
                        seg.len()
                    } else {
                        sb + ss as usize
                    };
                    if sid == ID_TRACKS {
                        found = Some((sb, sn));
                        break;
                    }
                    so = sn;
                }
                found
            };
            if let Some((ts, te)) = tracks_body {
                let tracks = &b[body + ts..body + te.min(b.len())];
                // 在 Tracks 里逐层找 Video 元素（TrackEntry→Video），取 PixelWidth/Height
                let mut pw: Option<u32> = None;
                let mut ph: Option<u32> = None;
                let mut to = 0usize;
                let mut in_video = false;
                while to < tracks.len() {
                    let (eid, el) = match read_id(tracks, to) {
                        Some(v) => v,
                        None => break,
                    };
                    let (es, esl) = match read_vint(tracks, to + el) {
                        Some(v) => v,
                        None => break,
                    };
                    let eb = to + el + esl;
                    let en = if es == 0 || eb + es as usize > tracks.len() {
                        tracks.len()
                    } else {
                        eb + es as usize
                    };
                    match eid {
                        ID_VIDEO => in_video = true,
                        _ if eid == ID_PIXEL_WIDTH && in_video => {
                            if let Some(v) = be_u16(tracks, eb) {
                                pw = Some(v as u32);
                            }
                        }
                        _ if eid == ID_PIXEL_HEIGHT && in_video => {
                            if let Some(v) = be_u16(tracks, eb) {
                                ph = Some(v as u32);
                            }
                        }
                        _ => {}
                    }
                    if pw.is_some() && ph.is_some() {
                        break;
                    }
                    to = en;
                }
                if let (Some(w), Some(h)) = (pw, ph) {
                    if w > 0 && h > 0 {
                        return Some((w, h));
                    }
                }
            }
        }
        o = next;
    }
    None
}

/// AVI：RIFF 容器。'RIFF' size 'AVI ' → 'hdrl' → 'avih'（主文件头）。
/// avih 偏移 32 字节处是 dwWidth（u32 LE），36 字节处是 dwHeight。
fn probe_avi(b: &[u8]) -> Option<(u32, u32)> {
    // 找 'avih' 标记
    let needle = b"avih";
    let pos = (0..b.len().saturating_sub(56)).find(|&i| &b[i..i + 4] == needle)?;
    // avih 后是 size(4) 再是内容；内容偏移 32/36 是宽高（LE u32）
    let content = pos + 4 + 4;
    let w = le_u32(b, content + 32)?;
    let h = le_u32(b, content + 36)?;
    if w > 0 && h > 0 && w < 100_000 && h < 100_000 {
        Some((w, h))
    } else {
        None
    }
}

/// 预解析视频分辨率（纯函数，无 AppHandle，可供 setup 阶段直接调用）。
/// 返回 None 表示无法解析（前端回退到兜底路径）。
pub fn probe_resolution(path: &str) -> Option<(u32, u32)> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // 读前 1MB 足够覆盖这些容器的头部（moov/tracks/avih 一般在文件前部）。
    // 注意：少数 mp4 的 moov 在文件尾部（流式优化），这种 probe 会失败 → 兜底。
    const HEAD: usize = 1_000_000;
    let b = read_head(path, HEAD).ok()?;

    match ext.as_str() {
        "mp4" | "m4v" | "mov" => probe_mp4(&b),
        "mkv" | "webm" => probe_mkv(&b),
        "avi" => probe_avi(&b),
        _ => None,
    }
}

/// 预解析视频分辨率（命令包装）。返回 None 表示无法解析（前端回退到兜底路径）。
#[tauri::command]
pub fn probe_video_resolution(path: String) -> Result<Option<(u32, u32)>, String> {
    Ok(probe_resolution(&path))
}
