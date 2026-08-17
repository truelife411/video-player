// 视频播放器 - Tauri 后端入口
mod commands;
mod db;
mod hash;
mod media;
mod pending_open;
mod probe;
mod startup_config;

use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_libmpv::MpvExt;

/// 关闭流程诊断日志（追加写入 %APPDATA%/com.hjf.videoplayer/shutdown.log）。
/// 用于定位"窗口关闭后进程不退出"的卡点：记录启动、CloseRequested 处理、
/// destroy 前后、ExitRequested 触发、强杀线程武装等关键事件。
fn log_event(msg: &str) {
    let Some(dir) = dirs::data_dir() else { return };
    let path = dir.join("com.hjf.videoplayer").join("shutdown.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(
            f,
            "[{} pid={}] {}",
            chrono::Local::now().format("%m-%d %H:%M:%S%.3f"),
            std::process::id(),
            msg
        );
    }
}

/// 用 TerminateProcess 立刻终止当前进程（不走正常退出流程）。
/// 与 ExitProcess 的根本区别：不执行任何 DLL 的 DllMain(DETACH)，直接终止所有线程，
/// 进程瞬间死亡——这是 Windows 上进程自杀最硬的方式，连安全软件也无法拦截"自己终止自己"。
/// 用途：彻底解决"退出流程中被第三方注入 DLL（搜狗输入法/StartAllBack）冻结/挂起"导致的进程残留。
#[cfg(target_os = "windows")]
fn force_kill_self() -> ! {
    unsafe {
        extern "system" {
            fn TerminateProcess(hProcess: *mut core::ffi::c_void, uExitCode: u32) -> i32;
            fn GetCurrentProcess() -> *mut core::ffi::c_void;
        }
        TerminateProcess(GetCurrentProcess(), 0);
    }
    // TerminateProcess 不应返回；若返回（理论上不可能），fallback 到正常退出。
    std::process::exit(0);
}

#[cfg(not(target_os = "windows"))]
fn force_kill_self() -> ! {
    std::process::exit(0);
}

/// 给定字幕文件路径，在同目录下查找同名的视频文件。
/// 例如 "D:/movies/Inception.srt" → 若存在 "D:/movies/Inception.mkv" 则返回其路径。
#[tauri::command]
fn find_sibling_video(sub_path: &str) -> Option<String> {
    let p = Path::new(sub_path);
    if !media::is_subtitle_path(p) || !p.is_file() {
        return None;
    }
    let dir = p.parent()?;
    let stem = p.file_stem()?.to_str()?;
    for ext in media::VIDEO_EXTS {
        let candidate: PathBuf = dir.join(format!("{}.{}", stem, ext));
        if candidate.is_file() {
            return candidate.to_str().map(|s| s.to_string());
        }
    }
    None
}

fn startup_file_from_args() -> Option<String> {
    media::video_file_from_args(std::env::args().skip(1))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log_event("app starting");
    // 启动 argv 里的视频路径：首启从命令行启动时使用（如双击文件且无已运行实例）。
    let startup_file = startup_file_from_args();

    let pending_open = pending_open::PendingOpen::default();
    if let Some(path) = startup_file.clone() {
        pending_open.put(path);
    }

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_libmpv::init())
        .manage(pending_open)
        .invoke_handler(tauri::generate_handler![
            find_sibling_video,
            pending_open::take_pending_open_file,
            commands::compute_video_hash,
            commands::register_video,
            commands::get_video,
            commands::save_play_position,
            commands::list_tag_types,
            commands::create_tag_type,
            commands::update_tag_type,
            commands::batch_update_video_tags,
            commands::delete_tag_type,
            commands::ensure_presets,
            commands::list_video_tags,
            commands::set_video_tag,
            commands::set_video_tag_values,
            commands::set_video_tag_if_absent,
            commands::set_system_tag_if_absent,
            commands::search_videos,
            commands::search_videos_page,
            commands::list_videos_by_stars,
            commands::reveal_in_explorer,
            commands::get_setting,
            commands::set_setting,
            commands::probe_video_resolution,
            commands::screenshots_dir,
            startup_config::get_single_instance,
            startup_config::set_single_instance,
        ])
        .on_window_event(|window, event| {
            // 终极方案：点 X 的瞬间不走任何正常退出流程，直接同步 TerminateProcess 自杀。
            //
            // 排查结论（详见 shutdown.log 心跳）：进程在 ExitRequested 后 1 秒内被第三方注入
            // DLL（搜狗输入法/StartAllBack）整体冻结，连强杀线程的 sleep 都醒不过来。
            // 所以不走退出流程这条路——prevent_close 后立刻自杀，进程瞬间死亡，
            // 窗口随之消失，不经过那个"退出即被冻结"的窗口期。
            //
            // 代价：进度不保存（前端每 5 秒定时保存，最多丢几秒）、WebView2 子进程可能
            // 偶发残留（不持视频文件句柄，下次播放不受影响）。
            // 好处：进程残留从机制上被根除，不再依赖任何"正常退出"能成功的前提。
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    log_event("CloseRequested: prevent_close + suicide (TerminateProcess)");
                    api.prevent_close();
                    force_kill_self(); // 不返回
                }
            }
        });

    if startup_config::load_before_tauri().single_instance {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(path) = media::video_file_from_args(argv.iter().skip(1)) {
                app.state::<pending_open::PendingOpen>().put(path);
                let _ = app.emit("open-file-available", ());
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }));
    }

    let app = builder
        .setup(move |app| {
            db::init(app.handle())?;

            // —— 消除启动跳变 ——
            // 窗口默认 visible:false（见 tauri.conf.json）。这里在 WebView2 渲染前：
            //   1) 若启动带视频：probe 分辨率 → 把窗口调到目标尺寸
            //   2) 调好尺寸后用 Rust 侧 show() 显示（不依赖前端 JS，避免死锁）
            // 这样窗口一出现就是正确尺寸，无白底大窗跳变。
            // 注意：必须用 Rust 侧 show——之前在前端 JS 里 show 会导致
            //       "visible:false 时 WebView2 不渲染 → JS 不执行 → 永不 show" 的死锁。
            //       Rust setup 是同步、在渲染前执行的，show 在这里最可靠。
            if let Some(w) = app.get_webview_window("main") {
                log_event("setup: main window found, showing");
                if let Some(file) = &startup_file {
                    if let Some((vw, vh)) = commands::probe_resolution(file) {
                        match startup_window_action(app.handle(), &w, vw, vh) {
                            WindowAction::Keep => {}
                            WindowAction::Maximize => {
                                let _ = w.maximize();
                            }
                            WindowAction::Resize(pw, ph) => {
                                let _ = w.set_size(tauri::PhysicalSize::new(pw, ph));
                                position_window_in_work_area(&w);
                            }
                        }
                    }
                }
                // 无论是否有视频、预 resize 是否成功，都显示窗口
                let _ = w.show();
                let _ = w.set_focus();
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("启动 Tauri 应用时出错");

    app.run(|_app_handle, event| {
        // ExitRequested 正常情况下不会到达（CloseRequested 已直接自杀），
        // 保留为兜底（其他退出路径，如 taskbar 右键退出）。
        if let RunEvent::ExitRequested { .. } = event {
            log_event("ExitRequested: fallback suicide");
            force_kill_self();
        }
    });
}

#[derive(Debug, PartialEq)]
enum WindowAction {
    Keep,
    Maximize,
    Resize(u32, u32),
}

fn setting(app: &tauri::AppHandle, key: &str) -> Option<String> {
    db::open(app)
        .ok()?
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .ok()
}

fn calculate_window_action(
    video_w: u32,
    video_h: u32,
    scale: f64,
    ui_height: u32,
    work_w: u32,
    work_h: u32,
    policy: &str,
) -> WindowAction {
    if policy == "keep" {
        return WindowAction::Keep;
    }
    if policy == "maximize" {
        return WindowAction::Maximize;
    }
    let work_w = work_w.max(1);
    let work_h = work_h.max(1);
    let ui_height = ui_height.min(work_h.saturating_sub(1));
    let available_h = work_h.saturating_sub(ui_height).max(1);
    let requested_scale = if policy == "fit" {
        f64::INFINITY
    } else {
        scale.max(0.01)
    };
    let actual_scale = requested_scale
        .min(work_w as f64 / video_w.max(1) as f64)
        .min(available_h as f64 / video_h.max(1) as f64);
    let content_w = ((video_w as f64 * actual_scale).floor() as u32)
        .max(1)
        .min(work_w);
    let content_h = ((video_h as f64 * actual_scale).floor() as u32)
        .max(1)
        .saturating_add(ui_height)
        .min(work_h);
    let min_w = 640.min(work_w);
    let min_h = 400.min(work_h);
    WindowAction::Resize(
        content_w.max(min_w).min(work_w),
        content_h.max(min_h).min(work_h),
    )
}

fn centered_position(
    work_x: i32,
    work_y: i32,
    work_w: u32,
    work_h: u32,
    outer_w: u32,
    outer_h: u32,
) -> (i32, i32) {
    let x = work_x + (work_w.saturating_sub(outer_w) / 2) as i32;
    let y = work_y + (work_h.saturating_sub(outer_h) / 2) as i32;
    (x, y)
}

fn position_window_in_work_area(window: &tauri::WebviewWindow) {
    let Some(monitor) = window.current_monitor().ok().flatten() else {
        return;
    };
    let Ok(outer) = window.outer_size() else {
        return;
    };
    let work = monitor.work_area();
    let (x, y) = centered_position(
        work.position.x,
        work.position.y,
        work.size.width,
        work.size.height,
        outer.width,
        outer.height,
    );
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

fn startup_window_action(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    video_w: u32,
    video_h: u32,
) -> WindowAction {
    let scale = setting(app, "window_scale")
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1.0);
    let policy = setting(app, "window_size_policy").unwrap_or_else(|| "video".into());
    let monitor = window.current_monitor().ok().flatten();
    let (work_w, work_h, scale_factor) = monitor
        .map(|monitor| {
            (
                monitor.work_area().size.width,
                monitor.work_area().size.height,
                monitor.scale_factor(),
            )
        })
        .unwrap_or((1920, 1080, 1.0));
    let frame = window
        .inner_size()
        .ok()
        .zip(window.outer_size().ok())
        .map(|(inner, outer)| {
            (
                outer.width.saturating_sub(inner.width),
                outer.height.saturating_sub(inner.height),
            )
        })
        .unwrap_or((
            (16.0 * scale_factor).round() as u32,
            (39.0 * scale_factor).round() as u32,
        ));
    let max_inner_w = work_w.saturating_sub(frame.0).max(1);
    let max_inner_h = work_h.saturating_sub(frame.1).max(1);
    let ui_height = (140.0 * scale_factor).round() as u32;
    calculate_window_action(
        video_w,
        video_h,
        scale,
        ui_height,
        max_inner_w,
        max_inner_h,
        &policy,
    )
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn fits_4k_inner_size_below_outer_work_area() {
        let action = calculate_window_action(3840, 2160, 1.0, 140, 1904, 1001, "video");
        assert_eq!(action, WindowAction::Resize(1530, 1001));
        let WindowAction::Resize(inner_w, inner_h) = action else {
            panic!("expected resize");
        };
        assert!(inner_w + 16 <= 1920);
        assert!(inner_h + 39 <= 1040);
    }

    #[test]
    fn centers_outer_window_inside_offset_work_area() {
        assert_eq!(
            centered_position(-1920, 40, 1920, 1040, 1546, 1040),
            (-1733, 40)
        );
    }

    #[test]
    fn only_explicit_policy_maximizes() {
        assert!(matches!(
            calculate_window_action(3840, 2160, 3.0, 172, 1920, 1040, "video"),
            WindowAction::Resize(_, _)
        ));
        assert_eq!(
            calculate_window_action(1920, 1080, 1.0, 172, 1920, 1040, "maximize"),
            WindowAction::Maximize
        );
        assert_eq!(
            calculate_window_action(1920, 1080, 1.0, 172, 1920, 1040, "keep"),
            WindowAction::Keep
        );
    }
}
