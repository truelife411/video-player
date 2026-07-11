// 视频播放器 - Tauri 后端入口
mod commands;
mod db;
mod file_assoc;
mod hash;

use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager};

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

/// 支持的视频扩展名白名单：启动参数（argv）只认这些后缀，避免把其他参数当文件路径。
const VIDEO_EXTS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "webm", "flv", "ts", "m4v", "wmv", "mpg", "mpeg", "vob",
];

/// 从命令行参数里取出第一个看起来像视频文件路径的项。
/// Windows 双击文件启动时，路径会作为 argv[1] 传入。
fn startup_file_from_args() -> Option<String> {
    let mut iter = std::env::args().skip(1);
    while let Some(a) = iter.next() {
        let lower = a.to_lowercase();
        if let Some(ext) = lower.rsplit('.').next() {
            if VIDEO_EXTS.contains(&ext) {
                return Some(a);
            }
        }
    }
    None
}

/// 前端 onMounted 调用：取出本次启动 argv 中的视频路径（取出即失效，不重复播放）。
#[tauri::command]
fn get_startup_file(state: tauri::State<'_, std::sync::Mutex<Option<String>>>) -> Option<String> {
    let mut guard = state.lock().ok()?;
    guard.take()
}

/// 读取 single_instance 设置（默认开启）。db 已初始化，直接同步读。
fn read_single_instance_setting(app: &tauri::AppHandle) -> bool {
    db::open(app)
        .and_then(|conn| {
            let v: Option<String> = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='single_instance'",
                    [],
                    |r| r.get(0),
                )
                .ok();
            Ok(v)
        })
        .ok()
        .flatten()
        .map(|s| s == "true" || s == "1")
        .unwrap_or(true) // 默认开启
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 启动 argv 里的视频路径：首启从命令行启动时使用（如双击文件且无已运行实例）。
    let startup_file = startup_file_from_args();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_libmpv::init())
        .manage(std::sync::Mutex::new(startup_file.clone()))
        .invoke_handler(tauri::generate_handler![
            find_sibling_video,
            get_startup_file,
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
            commands::get_setting,
            commands::set_setting,
            commands::probe_video_resolution,
            file_assoc::register_file_assoc,
            file_assoc::unregister_file_assoc,
            file_assoc::is_file_assoc_registered,
        ]);

    // 单实例模式：第二实例把 argv 文件路径转发给已运行实例（设置关闭时跳过，允许多开）。
    builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
        // argv[0] 是 exe 路径，从 argv[1] 起找视频路径
        let target = argv.iter().skip(1).find(|a| {
            let lower = a.to_lowercase();
            lower.rsplit('.').next().map_or(false, |ext| VIDEO_EXTS.contains(&ext))
        });
        if let Some(path) = target {
            // 转发给前端
            let _ = app.emit("open-file-argv", path);
        }
        // 把已运行实例的窗口拉到前台
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    }));

    builder
        .setup(move |app| {
            // 初始化数据库（schema + 预置标签）
            db::init(app.handle()).expect("数据库初始化失败");
            // 单实例模式设置：若用户关闭，重启后不再注册 single-instance 插件。
            // 注意：插件在 Builder 链上已固定，这里仅作为信息读取；实际"关闭"需重启生效
            // （由前端在切换时提示"下次启动生效"）。此处保留读取以备未来条件化安装。
            let _ = read_single_instance_setting(app.handle());

            // —— 消除启动跳变 ——
            // 窗口默认 visible:false（见 tauri.conf.json）。这里在 WebView2 渲染前：
            //   1) 若启动带视频：probe 分辨率 → 把窗口调到目标尺寸
            //   2) 调好尺寸后用 Rust 侧 show() 显示（不依赖前端 JS，避免死锁）
            // 这样窗口一出现就是正确尺寸，无白底大窗跳变。
            // 注意：必须用 Rust 侧 show——之前在前端 JS 里 show 会导致
            //       "visible:false 时 WebView2 不渲染 → JS 不执行 → 永不 show" 的死锁。
            //       Rust setup 是同步、在渲染前执行的，show 在这里最可靠。
            if let Some(w) = app.get_webview_window("main") {
                // 启动带视频：尝试预 resize（预解析失败则保持默认尺寸）
                if let Some(file) = &startup_file {
                    if let Some((vw, vh)) = commands::probe_resolution(file) {
                        // 预算物理像素目标尺寸（与前端 resizeWindowForVideo 完全一致，
                        // 含 windowScale 设置和 UI 高度补偿），避免 setup 与 openFrame 两次
                        // resize 因算法不同而产生「大跳小/小跳大」的二次跳变。
                        if let Some((pw, ph)) = target_window_phys_size(app.handle(), vw, vh) {
                            let _ = w.set_size(tauri::PhysicalSize::new(pw, ph));
                            let _ = w.center();
                        }
                    }
                }
                // 无论是否有视频、预 resize 是否成功，都显示窗口
                let _ = w.show();
                let _ = w.set_focus();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用时出错");
}

/// 根据视频分辨率推算窗口【物理像素】尺寸，与前端 resizeWindowForVideo 完全一致。
/// 含 windowScale 设置（从数据库读，默认 1.0）和 UI 高度补偿。
/// 返回 None 表示超出屏幕（交给前端 maximize）；Some((w,h)) 为物理像素。
fn target_window_phys_size(app: &tauri::AppHandle, video_w: u32, video_h: u32) -> Option<(u32, u32)> {
    // 读 windowScale 设置（与前端 persistSetting("window_scale") 一致）
    let scale: f64 = db::open(app)
        .ok()
        .and_then(|conn| {
            conn.query_row(
                "SELECT value FROM settings WHERE key='window_scale'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
        })
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(1.0);

    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Gdi::{
            GetDC, GetDeviceCaps, ReleaseDC, HORZRES, VERTRES, LOGPIXELSX,
        };
        unsafe {
            let hdc = GetDC(None);
            if !hdc.is_invalid() {
                // 屏幕物理像素 = 逻辑像素(HORZRES/VERTRES) × DPI缩放
                let log_w = GetDeviceCaps(hdc, HORZRES) as f64;
                let log_h = GetDeviceCaps(hdc, VERTRES) as f64;
                let dpi = GetDeviceCaps(hdc, LOGPIXELSX) as f64; // 96 = 100%
                let _ = ReleaseDC(None, hdc);
                let scale_factor = dpi / 96.0;
                let phys_screen_w = log_w * scale_factor;
                let phys_screen_h = log_h * scale_factor;
                // UI 高度补偿（物理像素）：与前端一致，172 逻辑像素 × DPI
                let ui_extra_phys = 172.0 * scale_factor;
                let need_w = (video_w as f64 * scale).round() as u32;
                let need_h = ((video_h as f64 + ui_extra_phys) * scale).round() as u32;
                // 超屏则交给前端 maximize
                if need_w as f64 > phys_screen_w || need_h as f64 > phys_screen_h {
                    return None;
                }
                return Some((need_w, need_h));
            }
        }
    }
    // 非 Windows 或查询失败：保守用视频原寸 × scale
    Some((
        (video_w as f64 * scale).round() as u32,
        ((video_h as f64 + 172.0) * scale).round() as u32,
    ))
}
