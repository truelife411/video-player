use rusqlite::{Connection, Transaction};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const SCHEMA_VERSION: i64 = 4;

pub fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取 app_data_dir: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("无法创建 app_data_dir: {e}"))?;
    Ok(dir.join("player.db"))
}

fn configure_connection(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(())
}

pub fn open(app: &AppHandle) -> Result<Connection, String> {
    let conn = Connection::open(db_path(app)?).map_err(|e| e.to_string())?;
    configure_connection(&conn).map_err(|e| e.to_string())?;
    Ok(conn)
}

pub fn init(app: &AppHandle) -> Result<(), String> {
    let mut conn = open(app)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| e.to_string())?;
    init_connection(&mut conn).map_err(|e| e.to_string())
}

pub fn init_connection(conn: &mut Connection) -> rusqlite::Result<()> {
    configure_connection(conn)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }

    let tx = conn.transaction()?;
    create_v1_schema(&tx)?;
    if version < 2 {
        migrate_v2(&tx)?;
    }
    if version < 3 {
        migrate_v3(&tx)?;
    }
    if version < 4 {
        migrate_v4(&tx)?;
    }
    ensure_presets_on(&tx)?;
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()
}

fn create_v1_schema(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS videos (
            hash TEXT PRIMARY KEY,
            file_name TEXT,
            file_path TEXT,
            extension TEXT,
            size_bytes INTEGER,
            modified_at INTEGER,
            play_position REAL DEFAULT 0,
            duration REAL,
            created_at INTEGER
        );
        CREATE TABLE IF NOT EXISTS tag_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL,
            value_type TEXT NOT NULL CHECK(value_type IN ('enum','free')),
            is_preset INTEGER DEFAULT 0,
            sort_order INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS tag_options (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type_id INTEGER NOT NULL REFERENCES tag_types(id) ON DELETE CASCADE,
            value TEXT NOT NULL,
            sort_order INTEGER DEFAULT 0,
            UNIQUE(type_id, value)
        );
        CREATE TABLE IF NOT EXISTS video_tags (
            video_hash TEXT NOT NULL REFERENCES videos(hash) ON DELETE CASCADE,
            type_id INTEGER NOT NULL REFERENCES tag_types(id) ON DELETE CASCADE,
            value_text TEXT NOT NULL,
            PRIMARY KEY (video_hash, type_id)
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )
}

fn has_column(tx: &Transaction<'_>, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut statement = tx.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(names.iter().any(|name| name == column))
}

fn migrate_v2(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !has_column(tx, "tag_types", "system_key")? {
        tx.execute_batch("ALTER TABLE tag_types ADD COLUMN system_key TEXT;")?;
    }
    if !has_column(tx, "tag_types", "is_multi")? {
        tx.execute_batch(
            "ALTER TABLE tag_types ADD COLUMN is_multi INTEGER NOT NULL DEFAULT 0 CHECK(is_multi IN (0,1));",
        )?;
    }
    tx.execute_batch(
        "
        UPDATE tag_types SET system_key='stars', is_preset=1, is_multi=0
          WHERE name='星级' AND system_key IS NULL;
        UPDATE tag_types SET system_key='quality', is_preset=1, is_multi=0
          WHERE name='画质' AND system_key IS NULL;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_tag_types_system_key
          ON tag_types(system_key) WHERE system_key IS NOT NULL;
        ",
    )
}

fn migrate_v3(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if has_column(tx, "video_tags", "sort_order")? {
        return Ok(());
    }
    tx.execute_batch(
        "
        CREATE TABLE video_tags_new (
            video_hash TEXT NOT NULL REFERENCES videos(hash) ON DELETE CASCADE,
            type_id INTEGER NOT NULL REFERENCES tag_types(id) ON DELETE CASCADE,
            value_text TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (video_hash, type_id, value_text)
        );
        INSERT INTO video_tags_new(video_hash,type_id,value_text,sort_order,created_at)
          SELECT video_hash,type_id,value_text,0,0 FROM video_tags;
        DROP TABLE video_tags;
        ALTER TABLE video_tags_new RENAME TO video_tags;
        ",
    )
}

fn migrate_v4(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "
        DROP INDEX IF EXISTS idx_video_tags_value;
        DROP INDEX IF EXISTS idx_video_tags_type;
        CREATE INDEX IF NOT EXISTS idx_video_tags_type_value_video
          ON video_tags(type_id,value_text,video_hash);
        CREATE INDEX IF NOT EXISTS idx_video_tags_video_type_order
          ON video_tags(video_hash,type_id,sort_order,value_text);
        CREATE INDEX IF NOT EXISTS idx_videos_modified_hash
          ON videos(modified_at DESC,hash);
        CREATE INDEX IF NOT EXISTS idx_tag_options_type_order
          ON tag_options(type_id,sort_order,id);
        ",
    )
}

pub fn ensure_presets(app: &AppHandle) -> Result<(), String> {
    let conn = open(app)?;
    ensure_presets_on(&conn).map_err(|e| e.to_string())
}

pub fn ensure_presets_on(conn: &Connection) -> rusqlite::Result<()> {
    ensure_system_tag(
        conn,
        "stars",
        "星级",
        1,
        &["1", "2", "3", "4", "5", "6", "7"],
    )?;
    ensure_system_tag(conn, "quality", "画质", 2, &["480p", "720p", "1080p", "4K"])?;
    Ok(())
}

fn ensure_system_tag(
    conn: &Connection,
    system_key: &str,
    default_name: &str,
    sort_order: i64,
    options: &[&str],
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO tag_types
         (name,value_type,is_preset,sort_order,system_key,is_multi)
         VALUES (?1,'enum',1,?2,?3,0)",
        rusqlite::params![default_name, sort_order, system_key],
    )?;
    conn.execute(
        "UPDATE tag_types SET value_type='enum',is_preset=1,is_multi=0
         WHERE system_key=?1",
        [system_key],
    )?;
    let type_id: i64 = conn.query_row(
        "SELECT id FROM tag_types WHERE system_key=?1",
        [system_key],
        |row| row.get(0),
    )?;
    for (index, value) in options.iter().enumerate() {
        conn.execute(
            "INSERT OR IGNORE INTO tag_options(type_id,value,sort_order) VALUES (?1,?2,?3)",
            rusqlite::params![type_id, value, index as i64],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_old_schema_and_preserves_tags() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE videos(hash TEXT PRIMARY KEY,file_name TEXT,file_path TEXT,extension TEXT,
              size_bytes INTEGER,modified_at INTEGER,play_position REAL DEFAULT 0,duration REAL,created_at INTEGER);
            CREATE TABLE tag_types(id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT UNIQUE NOT NULL,
              value_type TEXT NOT NULL,is_preset INTEGER DEFAULT 0,sort_order INTEGER DEFAULT 0);
            CREATE TABLE tag_options(id INTEGER PRIMARY KEY AUTOINCREMENT,type_id INTEGER NOT NULL,
              value TEXT NOT NULL,sort_order INTEGER DEFAULT 0,UNIQUE(type_id,value));
            CREATE TABLE video_tags(video_hash TEXT NOT NULL,type_id INTEGER NOT NULL,value_text TEXT NOT NULL,
              PRIMARY KEY(video_hash,type_id));
            CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
            INSERT INTO videos(hash,file_name,file_path) VALUES('h','v','p');
            INSERT INTO tag_types(name,value_type,is_preset,sort_order) VALUES('星级','enum',1,1);
            INSERT INTO video_tags(video_hash,type_id,value_text) VALUES('h',1,'5');
            ",
        )
        .unwrap();
        init_connection(&mut conn).unwrap();
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
        let key: String = conn
            .query_row("SELECT system_key FROM tag_types WHERE id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(key, "stars");
        conn.execute(
            "INSERT INTO video_tags(video_hash,type_id,value_text) VALUES('h',1,'6')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn initialization_is_idempotent_and_has_seven_stars() {
        let mut conn = Connection::open_in_memory().unwrap();
        init_connection(&mut conn).unwrap();
        init_connection(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tag_options o JOIN tag_types t ON t.id=o.type_id WHERE t.system_key='stars'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 7);
    }
}
