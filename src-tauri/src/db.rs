// SQLite 数据库初始化与 schema 管理
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// 获取数据库文件路径（app_data_dir/player.db）
pub fn db_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .expect("无法获取 app_data_dir");
    fs::create_dir_all(&dir).ok();
    dir.join("player.db")
}

/// 打开数据库连接
pub fn open(app: &AppHandle) -> rusqlite::Result<Connection> {
    let path = db_path(app);
    let conn = Connection::open(path)?;
    // 性能优化：WAL 模式
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

/// 初始化 schema + 预置数据（幂等，重复执行无害）
pub fn init(app: &AppHandle) -> rusqlite::Result<()> {
    let conn = open(app)?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS videos (
            hash          TEXT PRIMARY KEY,
            file_name     TEXT,
            file_path     TEXT,
            extension     TEXT,
            size_bytes    INTEGER,
            modified_at   INTEGER,
            play_position REAL DEFAULT 0,
            duration      REAL,
            created_at    INTEGER
        );

        CREATE TABLE IF NOT EXISTS tag_types (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            name          TEXT UNIQUE NOT NULL,
            value_type    TEXT NOT NULL CHECK(value_type IN ('enum','free')),
            is_preset     INTEGER DEFAULT 0,
            sort_order    INTEGER DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS tag_options (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            type_id       INTEGER NOT NULL REFERENCES tag_types(id) ON DELETE CASCADE,
            value         TEXT NOT NULL,
            sort_order    INTEGER DEFAULT 0,
            UNIQUE(type_id, value)
        );

        CREATE TABLE IF NOT EXISTS video_tags (
            video_hash    TEXT NOT NULL REFERENCES videos(hash) ON DELETE CASCADE,
            type_id       INTEGER NOT NULL REFERENCES tag_types(id) ON DELETE CASCADE,
            value_text    TEXT NOT NULL,
            PRIMARY KEY (video_hash, type_id)
        );
        CREATE INDEX IF NOT EXISTS idx_video_tags_value ON video_tags(value_text);
        CREATE INDEX IF NOT EXISTS idx_video_tags_type  ON video_tags(type_id);
        ",
    )?;

    // 预置标签类型：星级（枚举 1-5）、画质（枚举 480p/720p/1080p/4K）
    // 幂等插入
    conn.execute(
        "INSERT OR IGNORE INTO tag_types (name, value_type, is_preset, sort_order)
         VALUES ('星级','enum',1,1)",
        [],
    )?;
    let star_id: i64 = conn.query_row(
        "SELECT id FROM tag_types WHERE name='星级'",
        [],
        |r| r.get(0),
    )?;
    for (i, v) in ["1", "2", "3", "4", "5"].iter().enumerate() {
        conn.execute(
            "INSERT OR IGNORE INTO tag_options (type_id, value, sort_order) VALUES (?1,?2,?3)",
            rusqlite::params![star_id, v, i as i64],
        )?;
    }

    conn.execute(
        "INSERT OR IGNORE INTO tag_types (name, value_type, is_preset, sort_order)
         VALUES ('画质','enum',1,2)",
        [],
    )?;
    let quality_id: i64 = conn.query_row(
        "SELECT id FROM tag_types WHERE name='画质'",
        [],
        |r| r.get(0),
    )?;
    for (i, v) in ["480p", "720p", "1080p", "4K"].iter().enumerate() {
        conn.execute(
            "INSERT OR IGNORE INTO tag_options (type_id, value, sort_order) VALUES (?1,?2,?3)",
            rusqlite::params![quality_id, v, i as i64],
        )?;
    }

    Ok(())
}
