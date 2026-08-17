use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupConfig {
    #[serde(default = "default_single_instance")]
    pub single_instance: bool,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            single_instance: true,
        }
    }
}

fn default_single_instance() -> bool {
    true
}

fn config_path_from_dir(dir: &Path) -> PathBuf {
    dir.join("startup.json")
}

fn default_config_path() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| config_path_from_dir(&dir.join("com.hjf.videoplayer")))
}

pub fn load_before_tauri() -> StartupConfig {
    default_config_path()
        .as_deref()
        .map(load_from_path)
        .unwrap_or_default()
}

pub fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("无法创建应用数据目录: {e}"))?;
    Ok(config_path_from_dir(&dir))
}

pub fn load_from_path(path: &Path) -> StartupConfig {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn load(app: &AppHandle) -> Result<StartupConfig, String> {
    Ok(load_from_path(&config_path(app)?))
}

pub fn save(app: &AppHandle, config: &StartupConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let temp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&temp, bytes).map_err(|e| format!("无法写入启动配置: {e}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("无法替换启动配置: {e}"))?;
    }
    fs::rename(&temp, &path).map_err(|e| format!("无法保存启动配置: {e}"))
}

#[tauri::command]
pub fn get_single_instance(app: AppHandle) -> Result<bool, String> {
    Ok(load(&app)?.single_instance)
}

#[tauri::command]
pub fn set_single_instance(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut config = load(&app)?;
    config.single_instance = enabled;
    save(&app, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_config_falls_back_to_enabled() {
        let path = std::env::temp_dir().join(format!(
            "video-player-startup-invalid-{}.json",
            std::process::id()
        ));
        fs::write(&path, b"not json").unwrap();
        assert!(load_from_path(&path).single_instance);
        let _ = fs::remove_file(path);
    }
}
