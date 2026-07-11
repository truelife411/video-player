// Windows 文件关联：注册/取消注册 ProgID 与扩展名映射。
//
// 全部写入 HKCU\Software\Classes（用户级，无需管理员/UAC）。
// 注意：Windows 出于安全保护，无法静默改写"UserChoice"（真正的默认），
// 因此注册后应用只会出现在"打开方式"列表中，首次仍需用户手动选一次本应用。
//
// ProgID 沿用 tauri.conf.json 的 identifier（com.hjf.videoplayer）。

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;
    use tauri::AppHandle;
    use winreg::enums::*;
    use winreg::RegKey;
    use windows::Win32::UI::Shell::{
        SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_FLUSH, SHCNF_IDLIST,
    };

    /// ProgID（与应用 identifier 一致）。
    const PROG_ID: &str = "com.hjf.videoplayer";
    /// 在"打开方式"中显示的应用名。
    const FRIENDLY_NAME: &str = "视频播放器";
    /// 支持的视频扩展名（与前端 useMpv.ts 的 VIDEO_EXTENSIONS 保持一致）。
    const VIDEO_EXTS: &[&str] = &[
        "mkv", "mp4", "avi", "mov", "webm", "flv", "ts", "m4v", "wmv", "mpg", "mpeg", "vob",
    ];

    /// 当前 exe 路径（缓存，避免每次重新解析）。开发模式下指向 dev exe。
    fn exe_path() -> Result<&'static str, String> {
        static CACHE: OnceLock<Option<String>> = OnceLock::new();
        let v = CACHE.get_or_init(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
        });
        v.as_deref().ok_or_else(|| "无法获取当前 exe 路径".to_string())
    }

    /// 注册文件关联：写 ProgID + 各扩展名映射。
    pub fn register() -> Result<(), String> {
        let exe = exe_path()?;
        let classes = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey("Software\\Classes")
            .map_err(|e| format!("打开 Software\\Classes 失败: {e}"))?
            .0;

        // 1) ProgID 子键：shell\open\command、DefaultIcon、FriendlyAppName
        let (prog, _) = classes
            .create_subkey(PROG_ID)
            .map_err(|e| format!("创建 ProgID 失败: {e}"))?;
        prog.set_value("FriendlyAppName", &FRIENDLY_NAME)
            .ok();
        let (icon, _) = prog
            .create_subkey("DefaultIcon")
            .map_err(|e| format!("创建 DefaultIcon 失败: {e}"))?;
        icon.set_value("", &format!("{exe},0")).ok();
        let (cmd, _) = prog
            .create_subkey("shell\\open\\command")
            .map_err(|e| format!("创建 shell\\open\\command 失败: {e}"))?;
        // "%1" = 被双击的文件路径
        cmd.set_value("", &format!("\"{exe}\" \"%1\"")).ok();

        // 2) 每个扩展名指向 ProgID，并写 OpenWithProgids 让"打开方式"稳定列出
        for ext in VIDEO_EXTS {
            let dot = format!(".{ext}");
            let (sub, _) = classes
                .create_subkey(&dot)
                .map_err(|e| format!("创建 {dot} 失败: {e}"))?;
            sub.set_value("", &PROG_ID).ok();
            let (owp, _) = sub
                .create_subkey("OpenWithProgids")
                .map_err(|e| format!("创建 OpenWithProgids 失败: {e}"))?;
            owp.set_value(PROG_ID, &"").ok();
        }

        notify_assoc_changed();
        Ok(())
    }

    /// 取消注册：删除 ProgID 与各扩展名映射。
    pub fn unregister() -> Result<(), String> {
        let exe = exe_path()?; // 仅用于一致性校验，未命中也照删
        let _ = exe;
        let classes = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Software\\Classes", KEY_READ | KEY_WRITE)
            .map_err(|e| format!("打开 Software\\Classes 失败: {e}"))?;

        // 先删各扩展名下的本 ProgID 引用
        for ext in VIDEO_EXTS {
            let dot = format!(".{ext}");
            // 默认值清空
            if let Ok(sub) = classes.open_subkey(&dot) {
                let _ = sub.set_value("", &"");
            }
            // 删 OpenWithProgids 下的 ProgID 值
            if let Ok(owp) = classes.open_subkey(format!("{dot}\\OpenWithProgids").as_str()) {
                let _ = owp.delete_value(PROG_ID);
            }
        }

        // 再删整个 ProgID 子键（递归）
        let _ = classes.delete_subkey_all(PROG_ID);

        notify_assoc_changed();
        Ok(())
    }

    /// 是否已注册：探测 ProgID\shell\open\command 是否存在且指向当前 exe。
    pub fn is_registered() -> bool {
        let Ok(exe) = exe_path() else {
            return false;
        };
        let Ok(classes) =
            RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Classes")
        else {
            return false;
        };
        let Ok(cmd) = classes.open_subkey(format!("{PROG_ID}\\shell\\open\\command").as_str())
        else {
            return false;
        };
        let val: String = cmd.get_value("").unwrap_or_default();
        // 命令形如 "C:\...\video-player.exe" "%1"
        val.to_lowercase().contains(&exe.to_lowercase())
    }

    /// 通知资源管理器：文件关联已变更（图标/列表立即刷新）。
    fn notify_assoc_changed() {
        // SHCNF_FLUSH = 同步等待刷新完成
        unsafe {
            let _ = SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST | SHCNF_FLUSH, None, None);
        }
    }

    /// 注：保留 AppHandle 入参为将来可能的路径解析扩展，目前用 current_exe()。
    #[allow(dead_code)]
    fn _use_app_handle(_app: &AppHandle) {}
}

#[cfg(not(windows))]
mod imp {
    pub fn register() -> Result<(), String> {
        Err("文件关联仅支持 Windows".into())
    }
    pub fn unregister() -> Result<(), String> {
        Err("文件关联仅支持 Windows".into())
    }
    pub fn is_registered() -> bool {
        false
    }
}

// —— 对外命令（#[tauri::command] 包装，便于注册到 invoke_handler）——

#[tauri::command]
pub fn register_file_assoc() -> Result<(), String> {
    imp::register()
}

#[tauri::command]
pub fn unregister_file_assoc() -> Result<(), String> {
    imp::unregister()
}

#[tauri::command]
pub fn is_file_assoc_registered() -> bool {
    imp::is_registered()
}
