import { ref, onMounted, onUnmounted } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { pictureDir, join } from "@tauri-apps/api/path";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, PhysicalSize, currentMonitor } from "@tauri-apps/api/window";
import {
  init,
  command,
  setProperty,
  getProperty,
  observeProperties,
} from "tauri-plugin-libmpv-api";

// 支持的视频扩展名
const VIDEO_EXTENSIONS = [
  "mkv", "mp4", "avi", "mov", "webm", "flv", "ts", "m4v", "wmv", "mpg", "mpeg", "vob",
];

// 支持的字幕扩展名
const SUB_EXTENSIONS = ["srt", "ass", "ssa", "sub", "vtt", "lrc"];

// 倍速档位
export const SPEED_PRESETS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];

// 音轨/字幕轨信息
export interface Track {
  id: number;
  lang?: string;
  title?: string;
  codec?: string;
  default?: boolean;
  selected?: boolean;
  external?: boolean;
}

/**
 * mpv 播放器封装：管理初始化、状态同步、播放控制。
 * 所有与 libmpv 的交互都集中在这里，组件只消费响应式状态。
 */
export function useMpv() {
  // —— 响应式状态 ——
  const isReady = ref(false);
  const isPlaying = ref(false);
  const currentTime = ref(0);
  const duration = ref(0);
  const volume = ref(100);
  const isMuted = ref(false);
  const currentFile = ref<string>("");
  const currentFileName = ref<string>("");
  const speed = ref(1);
  // 轨道列表
  const audioTracks = ref<Track[]>([]);
  const subTracks = ref<Track[]>([]);
  const currentAudioId = ref<number>(1);
  const currentSubId = ref<number>(0); // 0 = 禁用
  // 画面
  const aspectRatio = ref<string>("Default"); // Default / 16:9 / 4:3 / ...
  const videoWidth = ref(0);
  const videoHeight = ref(0);
  // 画面变换：旋转角度（0/90/180/270）、水平/垂直翻转
  const videoRotate = ref(0);
  const hFlipped = ref(false);
  const vFlipped = ref(false);
  // AB 循环
  const abLoopA = ref<number | null>(null);
  const abLoopB = ref<number | null>(null);

  // 根据视频分辨率自动调整窗口大小。
  // 核心原则：视频像素 ↔ 屏幕物理像素 1:1（"原寸"= 一个视频像素对应一个屏幕像素）。
  // 所以全程用【物理像素】比较和设置，绕开 DPI 缩放造成的逻辑/物理像素换算误差。
  //
  // 策略：
  //   1) 原寸优先——480p 就占屏 1/4、1080p 就占屏一半多，居中显示；
  //   2) 一旦视频原寸（宽或高，含 UI 物理高度补偿）超过屏幕物理分辨率（如 4K），
  //      直接 maximize()——保证画面绝不超出屏幕。
  async function resizeWindowForVideo(w: number, h: number) {
    if (w <= 0 || h <= 0) return;
    try {
      const appWindow = getCurrentWindow();
      // 取屏幕物理像素
      let physScreenW: number;
      let physScreenH: number;
      let scaleFactor = 1;
      const monitor = await currentMonitor().catch(() => null);
      if (monitor && monitor.size && monitor.scaleFactor > 0) {
        physScreenW = monitor.size.width;
        physScreenH = monitor.size.height;
        scaleFactor = monitor.scaleFactor;
      } else {
        // 兜底：window.screen 是逻辑像素，反推物理像素
        scaleFactor = window.devicePixelRatio || 1;
        physScreenW = Math.round(window.screen.width * scaleFactor);
        physScreenH = Math.round(window.screen.height * scaleFactor);
      }
      // UI 高度补偿（逻辑像素）：控制栏~90 + 顶部文件名条~50 + 标题栏~32 = 172
      const UI_EXTRA_H_LOGICAL = 172;
      const uiExtraHPhys = Math.round(UI_EXTRA_H_LOGICAL * scaleFactor);
      // 用户缩放系数：在原寸基础上整体放大/缩小
      const scale = windowScale.value > 0 ? windowScale.value : 1;
      const needW = Math.round(w * scale);
      const needH = Math.round((h + uiExtraHPhys) * scale);

      if (needW > physScreenW || needH > physScreenH) {
        // 超屏：直接最大化，绝不出现画面在屏幕外
        await appWindow.maximize();
      } else {
        // 原寸：先解除可能的最大化态，再用物理像素设原寸并居中
        try {
          const isMax = await appWindow.isMaximized();
          if (isMax) await appWindow.unmaximize();
        } catch {
          /* 忽略 */
        }
        await appWindow.setSize(new PhysicalSize(needW, needH));
        await appWindow.center();
      }
    } catch (e) {
      console.warn("[自动调整窗口] 失败:", e);
    }
  }

  // 轮询读取视频分辨率并调整窗口（轮询比 watch 更可靠：不受 hash 计算与
  // observer 推送时序的竞态影响）。返回 Promise，在拿到分辨率并调整完窗口后 resolve。
  let resizePollTimer: ReturnType<typeof setInterval> | null = null;
  function stopResizePoll() {
    if (resizePollTimer) {
      clearInterval(resizePollTimer);
      resizePollTimer = null;
    }
  }
  // 等待 mpv 解析出真实分辨率（width/height），最多重试 40 次 * 250ms = 10 秒
  function waitForVideoResolution(): Promise<{ w: number; h: number } | null> {
    return new Promise((resolve) => {
      let attempts = 0;
      stopResizePoll();
      resizePollTimer = setInterval(async () => {
        attempts++;
        try {
          const w = await getProperty<number>("width", "int64").catch(() => 0);
          const h = await getProperty<number>("height", "int64").catch(() => 0);
          if (w > 0 && h > 0) {
            stopResizePoll();
            resolve({ w, h });
            return;
          }
        } catch {
          /* 忽略，继续重试 */
        }
        if (attempts > 40) {
          stopResizePoll();
          resolve(null);
        }
      }, 250);
    });
  }

  let unlisten: UnlistenFn | null = null;

  // 刷新轨道列表（文件加载后调用）
  // 注意：track-list 的 node 格式经 wrapper FFI 传递存在内存风险，
  // 改用 count + 逐项读字符串属性的方式，避开 node 读取。
  async function refreshTracks() {
    try {
      const count = await getProperty<number>("track-list/count", "int64").catch(() => 0);
      if (!count || count <= 0) return;
      const audios: Track[] = [];
      const subs: Track[] = [];
      for (let i = 0; i < count; i++) {
        const type = await getProperty<string>(`track-list/${i}/type`, "string").catch(
          () => ""
        );
        const id = await getProperty<number>(`track-list/${i}/id`, "int64").catch(() => -1);
        if (id < 0) continue;
        const lang = await getProperty<string>(`track-list/${i}/lang`, "string").catch(
          () => ""
        );
        const title = await getProperty<string>(`track-list/${i}/title`, "string").catch(
          () => ""
        );
        const track: Track = { id, lang: lang || undefined, title: title || undefined };
        if (type === "audio") audios.push(track);
        else if (type === "sub") subs.push(track);
      }
      audioTracks.value = audios;
      subTracks.value = subs;
    } catch (e) {
      console.error("[refreshTracks] 失败:", e);
    }
  }

  // —— 初始化 mpv ——
  async function initMpv() {
    await init({
      initialOptions: {
        vo: "gpu-next",
        hwdec: "auto-safe",
        "keep-open": "yes",
        "force-window": "yes",
        "audio-pitch-correction": "yes", // 倍速时保留音调
        "sub-auto": "fuzzy", // 自动加载同目录同名字幕（不要求完全同名）
        "screenshot-format": "png",
        "screenshot-directory": "~/Pictures/Screenshots",
        "screenshot-template": "%f-%P-%n", // 文件名-时间-序号
      },
      observedProperties: [
        ["pause", "flag"],
        ["time-pos", "double", "none"],
        ["duration", "double", "none"],
        ["filename", "string", "none"],
        ["volume", "double"],
        ["mute", "flag"],
        ["speed", "double"],
        ["video-params/w", "double"],
        ["video-params/h", "double"],
      ],
    });

    unlisten = await observeProperties(
      [
        ["pause", "flag"],
        ["time-pos", "double", "none"],
        ["duration", "double", "none"],
        ["filename", "string", "none"],
        ["volume", "double"],
        ["mute", "flag"],
        ["speed", "double"],
        ["video-params/w", "double"],
        ["video-params/h", "double"],
      ],
      ({ name, data }) => {
        switch (name) {
          case "pause":
            isPlaying.value = data !== true;
            break;
          case "time-pos":
            if (typeof data === "number") currentTime.value = data;
            break;
          case "duration":
            if (typeof data === "number") duration.value = data;
            break;
          case "filename":
            if (typeof data === "string") currentFileName.value = data;
            break;
          case "volume":
            if (typeof data === "number") volume.value = data;
            break;
          case "mute":
            isMuted.value = data === true;
            break;
          case "speed":
            if (typeof data === "number") speed.value = data;
            break;
          case "video-params/w":
            if (typeof data === "number") videoWidth.value = data;
            break;
          case "video-params/h":
            if (typeof data === "number") videoHeight.value = data;
            break;
        }
      }
    );
    isReady.value = true;
  }

  // —— 文件操作 ——
  // 当前视频的 hash（用于记忆进度 / 标签）
  const videoHash = ref<string>("");

  async function openFile(path: string) {
    // 先重置上一个视频残留的状态，避免新视频短暂显示旧的进度/轨道/分辨率
    currentTime.value = 0;
    duration.value = 0;
    audioTracks.value = [];
    subTracks.value = [];
    currentAudioId.value = 1;
    currentSubId.value = 0;
    videoWidth.value = 0;
    videoHeight.value = 0;
    abLoopA.value = null;
    abLoopB.value = null;
    // 重置画面变换（仅同步前端状态，mpv 侧由 loadfile 自动重置）
    videoRotate.value = 0;
    hFlipped.value = false;
    vFlipped.value = false;

    // 关键：以暂停态加载新视频，避免先以"原尺寸/上一视频尺寸"播放一小会。
    // 流程：loadfile(暂停) → 等待分辨率 → 调整窗口 → 取消暂停开始播放。
    await setProperty("pause", true);
    await command("loadfile", [path]);
    currentFile.value = path;

    // 等待 mpv 解析出真实分辨率，并据此调整窗口大小（这一步完成前视频保持暂停）
    const dim = await waitForVideoResolution();
    if (dim) {
      videoWidth.value = dim.w;
      videoHeight.value = dim.h;
      await resizeWindowForVideo(dim.w, dim.h);
    }

    // 窗口已就位，开始播放
    await setProperty("pause", false);
    isPlaying.value = true;

    // 后台注册视频（算 hash + 写元信息），并恢复上次播放进度
    invoke<string>("register_video", { path })
      .then(async (h) => {
        videoHash.value = h;
        // 自动识别文件名开头的★数并标注星级
        const starMatch = (path.split(/[\\/]/).pop() || "").match(/^★+/);
        if (starMatch) {
          const stars = Math.min(7, Math.max(1, starMatch[0].length));
          invoke("set_video_tag", { videoHash: h, typeId: 1, value: String(stars) })
            .catch((e) => console.error("[自动标注星级] 失败:", e));
        }
        // 自动检测视频分辨率并标注画质（仅当未手动设置时）
        setTimeout(async () => {
          try {
            if (videoWidth.value <= 0 || videoHeight.value <= 0) return;
            // 获取画质标签的 type_id（name='画质'）
            const types = await invoke<{ id: number; name: string }[]>("list_tag_types");
            const qualityType = types.find((t: { name: string }) => t.name === "画质");
            if (!qualityType) return;
            // 检查是否已手动设置
            const existingTags = await invoke<{ type_id: number; value: string }[]>(
              "list_video_tags",
              { videoHash: h }
            );
            const existingQuality = existingTags.find(
              (t: { type_id: number }) => t.type_id === qualityType.id
            );
            if (existingQuality && existingQuality.value) return; // 已有值，跳过
            // 根据分辨率映射画质
            const vh = videoHeight.value;
            let quality = "480p";
            if (vh >= 2160) quality = "4K";
            else if (vh >= 1080) quality = "1080p";
            else if (vh >= 720) quality = "720p";
            await invoke("set_video_tag", {
              videoHash: h,
              typeId: qualityType.id,
              value: quality,
            });
          } catch (e) {
            console.warn("[自动标注画质] 失败:", e);
          }
        }, 1200);
        try {
          const info = await invoke<{ play_position: number; duration: number } | null>(
            "get_video",
            { hash: h }
          );
          // 仅在"从上次位置"模式下恢复进度；"从头开始"模式跳过
          if (
            resumeMode.value === "resume" &&
            info &&
            info.play_position > 5
          ) {
            // 恢复到上次进度（跳过开头 5 秒以内的）
            await command("seek", [info.play_position, "absolute"]);
          }
        } catch (e) {
          console.warn("[记忆进度] 恢复失败:", e);
        }
      })
      .catch((e) => console.warn("[注册视频] 失败:", e));
    // 文件加载后刷新轨道列表（稍等 mpv 解析）
    setTimeout(() => refreshTracks(), 800);
  }

  // 保存当前进度（供定时器和退出时调用）
  async function saveProgress() {
    if (!videoHash.value || currentTime.value <= 0) return;
    try {
      await invoke("save_play_position", {
        hash: videoHash.value,
        position: currentTime.value,
        duration: duration.value,
      });
    } catch (e) {
      console.warn("[保存进度] 失败:", e);
    }
  }

  async function openFileDialog() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "视频", extensions: VIDEO_EXTENSIONS }],
    });
    if (typeof selected === "string") await openFile(selected);
  }

  // 外挂字幕：用 select 标志加载后立即选中
  async function loadSubtitle(path: string) {
    await command("sub-add", [path, "select"]);
    await refreshTracks();
  }

  async function loadSubtitleDialog() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "字幕", extensions: SUB_EXTENSIONS }],
    });
    if (typeof selected === "string") await loadSubtitle(selected);
  }

  // 拖拽文件智能处理：视频→播放；字幕→找同目录同名视频播放（字幕由 sub-auto=fuzzy 自动加载，避免重复）
  async function openDroppedFile(path: string) {
    const ext = path.split(".").pop()?.toLowerCase() || "";
    if (SUB_EXTENSIONS.includes(ext)) {
      // 字幕：调 Rust 命令找同名视频
      try {
        const videoPath = await invoke<string | null>("find_sibling_video", { subPath: path });
        if (videoPath) {
          // 只播放视频；同名字幕由 sub-auto=fuzzy 自动加载，不手动重复加载
          await openFile(videoPath);
        } else {
          // 找不到同名视频，仅加载字幕到当前播放
          if (currentFile.value) await loadSubtitle(path);
        }
      } catch (e) {
        console.error("[openDroppedFile] 字幕处理失败:", e);
      }
    } else if (VIDEO_EXTENSIONS.includes(ext)) {
      await openFile(path);
    }
  }

  // —— 播放控制 ——
  async function togglePlay() {
    if (!currentFile.value) return;
    try {
      const paused = await getProperty<boolean>("pause", "flag");
      await setProperty("pause", !paused);
    } catch (e) {
      console.error("[togglePlay] 失败:", e);
    }
  }

  async function seekTo(seconds: number) {
    await command("seek", [seconds, "absolute"]);
  }

  async function seekBy(delta: number) {
    await command("seek", [delta, "relative"]);
  }

  // —— 音量 ——
  async function setVolume(v: number) {
    await setProperty("volume", v);
    if (v > 0 && isMuted.value) await setProperty("mute", false);
  }

  async function toggleMute() {
    await setProperty("mute", !isMuted.value);
  }

  // —— 倍速 ——
  async function setSpeed(s: number) {
    await setProperty("speed", s);
  }

  // —— 轨道切换（用 set 命令更可靠，禁用传 "no"）——
  async function setAudioTrack(id: number) {
    await command("set", ["aid", String(id)]);
    currentAudioId.value = id;
  }

  async function setSubTrack(id: number) {
    // 0 = 禁用字幕，传 "no"；否则传轨道 id
    const val = id === 0 ? "no" : String(id);
    await command("set", ["sid", asSubValue(val)]);
    currentSubId.value = id;
  }

  // 帮助：mpv set 命令的 sid 值直接用字符串
  function asSubValue(v: string): string {
    return v;
  }

  // —— 截图：用 screenshot-to-file 指定路径，返回保存路径供前端提示 ——
  async function screenshot(includeSubtitles = true): Promise<string> {
    // 生成文件名：截图_视频名_时间戳.png
    const base = currentFileName.value
      ? currentFileName.value.replace(/\.[^.]+$/, "")
      : "screenshot";
    const now = new Date();
    const ts = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}_${pad(
      now.getHours()
    )}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
    const dir = await pictureDir();
    const filePath = await join(dir, `${base}_${ts}.png`);
    // screenshot-to-file <路径> <模式(subtitles|video|window)>
    await command("screenshot-to-file", [filePath, includeSubtitles ? "subtitles" : "video"]);
    return filePath;
  }

  function pad(n: number): string {
    return n.toString().padStart(2, "0");
  }

  // —— A-B 循环 ——
  async function setAbLoopA() {
    const t = currentTime.value;
    await setProperty("ab-loop-a", t);
    abLoopA.value = t;
  }
  async function setAbLoopB() {
    const t = currentTime.value;
    await setProperty("ab-loop-b", t);
    abLoopB.value = t;
  }
  async function clearAbLoop() {
    await setProperty("ab-loop-a", "no");
    await setProperty("ab-loop-b", "no");
    abLoopA.value = null;
    abLoopB.value = null;
  }

  // —— 逐帧（frame-back-step 要求先暂停；frame-step 在播放/暂停均可）——
  async function frameBackStep() {
    // 确保暂停，否则后退无效
    await setProperty("pause", true);
    await command("frame-back-step");
  }
  async function frameStep() {
    // 播放中逐帧会立即恢复，先暂停更直观
    await setProperty("pause", true);
    await command("frame-step");
  }

  // —— 画面 ——
  async function setAspectRatio(r: string) {
    // "Default" → 恢复原始；否则设为指定比例
    await setProperty("video-aspect-override", r === "Default" ? "no" : r);
    aspectRatio.value = r;
  }

  // 翻转/旋转。video-rotate 是 mpv 的可写属性，但部分封装对其用 setProperty 不生效，
  // 改用 command("set", [...]) 更可靠。
  async function toggleHFlip() {
    await command("vf", ["toggle", "hflip"]);
    hFlipped.value = !hFlipped.value;
  }
  async function toggleVFlip() {
    await command("vf", ["toggle", "vflip"]);
    vFlipped.value = !vFlipped.value;
  }
  // 顺时针旋转 90°（0→90→180→270→0 循环）
  async function rotate90() {
    videoRotate.value = (videoRotate.value + 90) % 360;
    try {
      await command("set", ["video-rotate", String(videoRotate.value)]);
    } catch (e) {
      console.error("[rotate90] 失败:", e);
      // 回退到 setProperty
      await setProperty("video-rotate", videoRotate.value);
    }
  }
  // 逆时针旋转 90°
  async function rotateMinus90() {
    videoRotate.value = (videoRotate.value + 270) % 360;
    try {
      await command("set", ["video-rotate", String(videoRotate.value)]);
    } catch (e) {
      console.error("[rotateMinus90] 失败:", e);
      await setProperty("video-rotate", videoRotate.value);
    }
  }
  // 还原画面变换（旋转/翻转全部复位）
  async function resetTransform() {
    if (videoRotate.value !== 0) {
      try {
        await command("set", ["video-rotate", "0"]);
      } catch {
        await setProperty("video-rotate", 0);
      }
      videoRotate.value = 0;
    }
    if (hFlipped.value) {
      await command("vf", ["toggle", "hflip"]);
      hFlipped.value = false;
    }
    if (vFlipped.value) {
      await command("vf", ["toggle", "vflip"]);
      vFlipped.value = false;
    }
  }

  // 色彩：brightness -100~100, contrast 0~100(scaled), saturation 0~100(scaled)
  async function setBrightness(v: number) {
    await setProperty("brightness", v);
  }
  async function setContrast(v: number) {
    await setProperty("contrast", v);
  }
  async function setSaturation(v: number) {
    await setProperty("saturation", v);
  }

  // 定时保存进度（每 5 秒）
  let progressTimer: ReturnType<typeof setInterval> | null = null;

  onMounted(() => {
    initMpv().catch((e) => console.error("mpv init failed:", e));
    progressTimer = setInterval(() => saveProgress(), 5000);
  });

  onUnmounted(() => {
    unlisten?.();
    if (progressTimer) clearInterval(progressTimer);
    stopResizePoll();
    // 退出时最后保存一次（不等待，best-effort）
    saveProgress();
  });

  // 快进/快退时间（秒），可通过设置面板修改
  const skipSeconds = ref(10);

  // 窗口缩放系数：用户可调，打开视频时窗口在原寸基础上乘以此系数；超出屏幕仍自动最大化
  const windowScale = ref(1);

  // 新视频播放起点："start"=从头、"resume"=从上次位置
  const resumeMode = ref<"start" | "resume">("resume");

  // 关闭当前文件
  async function closeFile() {
    await command("stop");
    stopResizePoll();
    currentFile.value = "";
    currentFileName.value = "";
    videoHash.value = "";
    isPlaying.value = false;
    currentTime.value = 0;
    duration.value = 0;
    audioTracks.value = [];
    subTracks.value = [];
    videoWidth.value = 0;
    videoHeight.value = 0;
    abLoopA.value = null;
    abLoopB.value = null;
  }

  return {
    // 状态
    isReady, isPlaying, currentTime, duration, volume, isMuted,
    currentFile, currentFileName, speed, videoHash,
    audioTracks, subTracks, currentAudioId, currentSubId,
    aspectRatio, abLoopA, abLoopB, skipSeconds, windowScale, resumeMode,
    videoWidth, videoHeight, videoRotate, hFlipped, vFlipped,
    // 文件
    openFileDialog, openFile, closeFile, loadSubtitle, loadSubtitleDialog, openDroppedFile,
    // 播放
    togglePlay, seekTo, seekBy, setVolume, toggleMute, setSpeed, saveProgress,
    // 轨道
    setAudioTrack, setSubTrack, refreshTracks,
    // 截图
    screenshot,
    // AB循环/逐帧
    setAbLoopA, setAbLoopB, clearAbLoop, frameBackStep, frameStep,
    // 画面
    setAspectRatio, toggleHFlip, toggleVFlip, rotate90, rotateMinus90, resetTransform,
    setBrightness, setContrast, setSaturation,
  };
}
