import { ref, computed, onMounted, onUnmounted } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { join } from "@tauri-apps/api/path";
import { invoke } from "@tauri-apps/api/core";
import { VIDEO_EXTENSIONS, SUBTITLE_EXTENSIONS, extensionOf, leadingStarCount, qualityFromHeight } from "../utils/video";
import { getCurrentWindow, PhysicalPosition, PhysicalSize, currentMonitor } from "@tauri-apps/api/window";
import { buildScreenshotFileName } from "../utils/screenshot";
import { calculateWindowSize, type WindowSizePolicy } from "../utils/windowSizing";
import { LoadGate } from "../utils/loadGate";
import {
  init,
  command,
  setProperty,
  getProperty,
  observeProperties,
  listenEvents,
  destroy,
} from "tauri-plugin-libmpv-api";

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
  const settingsReady = ref(false);
  const isPlaying = ref(false);
  const currentTime = ref(0);
  const duration = ref(0);
  const volume = ref(100);
  const isMuted = ref(false);
  const playbackState = ref<"idle" | "preparing" | "loading" | "restoring" | "playing" | "paused" | "buffering" | "ended" | "failed">("idle");
  const playbackError = ref("");
  const pausedForCache = ref(false);
  const cacheBufferingState = ref(100);
  const demuxerCacheDuration = ref(0);
  const bufferedUntil = computed(() => Math.min(duration.value, currentTime.value + Math.max(0, demuxerCacheDuration.value)));
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
  const abLoopBusy = ref(false);
  const transformBusy = ref(false);
  const screenshotBusy = ref(false);
  const windowSizePolicy = ref<WindowSizePolicy>("video");
  let hwdecBeforeFlip = "auto-safe";

  // 根据视频分辨率自动调整窗口大小。
  // 核心原则：视频像素 ↔ 屏幕物理像素 1:1（"原寸"= 一个视频像素对应一个屏幕像素）。
  // 所以全程用【物理像素】比较和设置，绕开 DPI 缩放造成的逻辑/物理像素换算误差。
  //
  // 策略：按用户缩放优先显示；若视频区域超过屏幕可用空间，则等比缩小到
  // 能容纳的最大普通窗口。只有显式选择“最大化”策略才进入系统最大化状态。
  async function resizeWindowForVideo(w: number, h: number) {
    if (w <= 0 || h <= 0) return;
    try {
      const appWindow = getCurrentWindow();
      let workX = 0;
      let workY = 0;
      let physScreenW: number;
      let physScreenH: number;
      let scaleFactor = 1;
      const monitor = await currentMonitor().catch(() => null);
      if (monitor && monitor.workArea && monitor.scaleFactor > 0) {
        workX = monitor.workArea.position.x;
        workY = monitor.workArea.position.y;
        physScreenW = monitor.workArea.size.width;
        physScreenH = monitor.workArea.size.height;
        scaleFactor = monitor.scaleFactor;
      } else {
        scaleFactor = window.devicePixelRatio || 1;
        workX = Math.round(window.screenX * scaleFactor);
        workY = Math.round(window.screenY * scaleFactor);
        physScreenW = Math.round(window.screen.availWidth * scaleFactor);
        physScreenH = Math.round(window.screen.availHeight * scaleFactor);
      }
      const [inner, outer] = await Promise.all([
        appWindow.innerSize().catch(() => null),
        appWindow.outerSize().catch(() => null),
      ]);
      const frameWidth = inner && outer ? Math.max(0, outer.width - inner.width) : Math.round(16 * scaleFactor);
      const frameHeight = inner && outer ? Math.max(0, outer.height - inner.height) : Math.round(39 * scaleFactor);
      const maxInnerWidth = Math.max(1, physScreenW - frameWidth);
      const maxInnerHeight = Math.max(1, physScreenH - frameHeight);
      // 只计算 WebView 内部 UI；原生标题栏和边框由 frameWidth/frameHeight 单独扣除。
      const UI_EXTRA_H_LOGICAL = 140;
      const uiExtraHPhys = Math.round(UI_EXTRA_H_LOGICAL * scaleFactor);
      // 用户缩放系数：在原寸基础上整体放大/缩小
      const scale = windowScale.value > 0 ? windowScale.value : 1;
      const result = calculateWindowSize({
        videoWidth: w,
        videoHeight: h,
        scale,
        uiHeight: uiExtraHPhys,
        workWidth: maxInnerWidth,
        workHeight: maxInnerHeight,
        policy: windowSizePolicy.value,
      });
      if (result.action === "keep") return;
      if (result.action === "maximize") {
        await appWindow.maximize();
      } else {
        try {
          if (await appWindow.isMaximized()) await appWindow.unmaximize();
        } catch { /* 忽略 */ }
        await appWindow.setSize(new PhysicalSize(result.width, result.height));
        const resizedOuter = await appWindow.outerSize().catch(() => ({
          width: result.width + frameWidth,
          height: result.height + frameHeight,
        }));
        const x = workX + Math.max(0, Math.floor((physScreenW - resizedOuter.width) / 2));
        const y = workY + Math.max(0, Math.floor((physScreenH - resizedOuter.height) / 2));
        await appWindow.setPosition(new PhysicalPosition(x, y));
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
  let unlistenEvents: UnlistenFn | null = null;
  const loadGate = new LoadGate();

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
        const codec = await getProperty<string>(`track-list/${i}/codec`, "string").catch(() => "");
        const selected = await getProperty<boolean>(`track-list/${i}/selected`, "flag").catch(() => false);
        const isDefault = await getProperty<boolean>(`track-list/${i}/default`, "flag").catch(() => false);
        const external = await getProperty<boolean>(`track-list/${i}/external`, "flag").catch(() => false);
        const track: Track = { id, lang: lang || undefined, title: title || undefined, codec: codec || undefined, selected, default: isDefault, external };
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
        ["paused-for-cache", "flag"],
        ["cache-buffering-state", "double"],
        ["demuxer-cache-duration", "double"],
        ["aid", "string", "none"],
        ["sid", "string", "none"],
        ["track-list/count", "int64"],
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
        ["paused-for-cache", "flag"],
        ["cache-buffering-state", "double"],
        ["demuxer-cache-duration", "double"],
        ["aid", "string", "none"],
        ["sid", "string", "none"],
        ["track-list/count", "int64"],
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
          case "paused-for-cache":
            pausedForCache.value = data === true;
            if (data === true) playbackState.value = "buffering";
            else if (currentFile.value) playbackState.value = isPlaying.value ? "playing" : "paused";
            break;
          case "cache-buffering-state":
            if (typeof data === "number") cacheBufferingState.value = data;
            break;
          case "demuxer-cache-duration":
            if (typeof data === "number") demuxerCacheDuration.value = data;
            break;
          case "aid":
            currentAudioId.value = typeof data === "string" && Number.isFinite(Number(data)) ? Number(data) : 0;
            break;
          case "sid":
            currentSubId.value = typeof data === "string" && Number.isFinite(Number(data)) ? Number(data) : 0;
            break;
          case "track-list/count":
            void refreshTracks();
            break;
        }
      }
    );
    unlistenEvents = await listenEvents(async (event) => {
      if (event.event === "start-file") {
        playbackState.value = "loading";
        loadGate.handle("start-file");
      } else if (event.event === "file-loaded") {
        playbackState.value = "restoring";
        const loadedPath = await getProperty<string>("path", "string").catch(() => "");
        loadGate.handle("file-loaded", loadedPath);
        void refreshTracks();
      } else if (event.event === "end-file") {
        if (event.reason === "error") {
          playbackState.value = "failed";
          playbackError.value = `无法播放该文件（mpv 错误 ${event.error}）`;
          const failedPath = await getProperty<string>("path", "string").catch(() => "");
          loadGate.handle("error", failedPath, new Error(playbackError.value));
        } else if (event.reason === "eof") {
          playbackState.value = "ended";
          void saveProgress(true);
        }
      } else if (event.event === "audio-reconfig" || event.event === "video-reconfig") {
        void refreshTracks();
      } else if (event.event === "playback-restart" && currentFile.value) {
        playbackState.value = isPlaying.value ? "playing" : "paused";
      } else if (event.event === "shutdown") {
        playbackState.value = "failed";
        playbackError.value = "播放器内核已停止";
      }
    });
    isReady.value = true;
  }

  // —— 文件操作 ——
  // 当前视频的 hash（用于记忆进度 / 标签）
  const videoHash = ref<string>("");
  // 是否正在打开视频（供前端显示加载提示，并掩盖窗口尺寸调整过程）
  const isOpening = ref(false);

  let openToken = 0;

  async function openFile(path: string) {
    await saveProgress();
    const token = ++openToken;
    videoHash.value = "";
    playbackError.value = "";
    playbackState.value = "preparing";
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
    // 重置画面变换 + 播放速度：loadfile 未必重置所有项，需显式重置 mpv 侧
    // （设置不记忆，每次打开都复原：旋转/翻转/速度全部归零）。
    videoRotate.value = 0;
    hFlipped.value = false;
    vFlipped.value = false;
    speed.value = 1;
    await setProperty("video-rotate", 0).catch(() => {});
    if (token !== openToken) return;
    await setProperty("vf", "").catch(() => {});
    if (token !== openToken) return;
    await setProperty("hwdec", "auto-safe").catch(() => {});
    if (token !== openToken) return;
    await setProperty("speed", 1).catch(() => {});
    if (token !== openToken) return;

    // 关键：以暂停态加载新视频，避免先以"原尺寸/上一视频尺寸"播放一小会。
    isOpening.value = true;
    const appWindow = getCurrentWindow();
    try {
      const registration = invoke<string>("register_video", { path });
      await setProperty("pause", true);
      if (token !== openToken) return;
      const loaded = loadGate.wait(token, path, 15000);
      await command("loadfile", [path]);
      await loaded;
      if (token !== openToken) return;
      currentFile.value = path;
      const h = await registration;
      if (token !== openToken) return;
      videoHash.value = h;
      const info = await invoke<{ play_position: number; duration: number } | null>("get_video", { hash: h });

      if (resumeMode.value === "resume" && info?.play_position && info.play_position > 5) {
        const realDuration = (await getProperty<number>("duration", "double").catch(() => duration.value)) || info.duration;
        const target = Math.min(info.play_position, realDuration || info.play_position);
        if (!(realDuration > 0 && (target / realDuration >= 0.95 || realDuration - target <= 30))) {
          playbackState.value = "restoring";
          await command("seek", [target, "absolute", "exact"]);
        }
      }

      // loadfile 之后再次重置旋转：loadfile 会把 video-rotate 设回文件元数据值，
      // 必须在 loadfile 完成后重置才能确保新视频从 0° 开始（旋转不记忆）。
      await command("set", ["video-rotate", "0"]).catch(() => {});
      videoRotate.value = 0;

      // 消除窗口跳变的「两层」策略：
      //   第 1 层（主）：Rust 预解析容器头（probe_video_resolution），loadfile 之后
      //                 拿到分辨率就立刻 resize，窗口直接是目标尺寸，无跳变。
      //   第 2 层（兜底）：预解析失败时（不支持的格式 / moov 在文件尾部 / 损坏），
      //                 隐藏窗口 → 等 mpv 解析出分辨率 → resize → 显示，用加载提示掩盖。
      let probed: { w: number; h: number } | null = null;
      try {
        const r = await invoke<[number, number] | null>("probe_video_resolution", { path });
        if (token !== openToken) return;
        if (r && r[0] > 0 && r[1] > 0) probed = { w: r[0], h: r[1] };
      } catch (e) {
        console.warn("[预解析分辨率] 失败，回退兜底:", e);
      }

      if (probed) {
        // 主路径：已拿到分辨率，直接 resize（loadfile 并行进行），一步到位
        videoWidth.value = probed.w;
        videoHeight.value = probed.h;
        await resizeWindowForVideo(probed.w, probed.h);
        if (token !== openToken) return;
      } else {
        // 兜底路径：隐藏窗口，等 mpv 解析分辨率后再显示（避免跳变）
        try {
          await appWindow.hide();
        } catch {
          /* 忽略 */
        }
        const dim = await waitForVideoResolution();
        if (token !== openToken) return;
        if (dim) {
          videoWidth.value = dim.w;
          videoHeight.value = dim.h;
          await resizeWindowForVideo(dim.w, dim.h);
        }
      }
    } catch (e) {
      if (token !== openToken) return;
      console.error("[openFile] 失败:", e);
      playbackState.value = "failed";
      playbackError.value = e instanceof Error ? e.message : String(e);
      isPlaying.value = false;
      throw e;
    } finally {
      // 兜底路径隐藏过窗口：确保恢复显示（即使超时拿不到分辨率也不卡黑屏）。
      // show 对已可见窗口是 no-op，主路径/异常路径都会安全跳过。
      if (token === openToken) {
        try {
          await appWindow.show();
          await appWindow.setFocus();
        } catch {
          /* 忽略 */
        }
        isOpening.value = false;
      }
    }

    // 窗口已就位，开始播放
    if (token !== openToken) return;
    await setProperty("pause", false);
    if (token !== openToken) return;
    isPlaying.value = true;
    playbackState.value = "playing";

    // 自动标签由后端按稳定 system_key 原子写入，避免依赖标签列表加载时序。
    void (async () => {
      if (token !== openToken || !videoHash.value) return;
      const hash = videoHash.value;
      const stars = leadingStarCount(path);
      if (stars) {
        await invoke("set_system_tag_if_absent", {
          videoHash: hash,
          systemKey: "stars",
          value: String(stars),
        });
      }
      if (videoHeight.value > 0) {
        await invoke("set_system_tag_if_absent", {
          videoHash: hash,
          systemKey: "quality",
          value: qualityFromHeight(videoHeight.value),
        });
      }
    })().catch((e) => console.warn("[自动标签] 失败:", e));
    void refreshTracks();
  }

  // 保存当前进度（供定时器和退出时调用）
  async function saveProgress(completed = false) {
    const hash = videoHash.value;
    if (!hash) return;
    let position = currentTime.value;
    let total = duration.value;
    try {
      position = (await getProperty<number>("time-pos", "double").catch(() => position)) ?? position;
      total = (await getProperty<number>("duration", "double").catch(() => total)) ?? total;
    } catch { /* 使用观察值 */ }
    if (completed || (total > 0 && (position / total >= 0.95 || total - position <= 30))) position = 0;
    try {
      await invoke("save_play_position", { hash, position, duration: total });
    } catch (e) {
      console.warn("[保存进度] 失败:", e);
    }
  }

  async function openFileDialog() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "视频", extensions: [...VIDEO_EXTENSIONS] }],
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
      filters: [{ name: "字幕", extensions: [...SUBTITLE_EXTENSIONS] }],
    });
    if (typeof selected === "string") await loadSubtitle(selected);
  }

  // 拖拽文件智能处理：视频→播放；字幕→找同目录同名视频播放（字幕由 sub-auto=fuzzy 自动加载，避免重复）
  async function openDroppedFile(path: string) {
    const ext = extensionOf(path);
    if (SUBTITLE_EXTENSIONS.includes(ext as (typeof SUBTITLE_EXTENSIONS)[number])) {
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
    } else if (VIDEO_EXTENSIONS.includes(ext as (typeof VIDEO_EXTENSIONS)[number])) {
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

  async function replay() {
    if (!currentFile.value) return;
    await command("seek", [0, "absolute", "exact"]);
    await setProperty("pause", false);
    currentTime.value = 0;
    isPlaying.value = true;
    playbackState.value = "playing";
  }

  async function seekTo(seconds: number) {
    await command("seek", [seconds, "absolute"]);
  }

  async function seekBy(delta: number) {
    await command("seek", [delta, "relative"]);
  }

  // —— 音量 ——
  let volumePersistTimer: ReturnType<typeof setTimeout> | null = null;
  async function persistAudioSettings() {
    await Promise.all([
      invoke("set_setting", { key: "volume", value: String(Math.round(volume.value)) }),
      invoke("set_setting", { key: "muted", value: String(isMuted.value) }),
    ]).catch((e) => console.warn("[音量设置] 保存失败:", e));
  }

  async function setVolume(v: number) {
    await setProperty("volume", v);
    if (v > 0 && isMuted.value) await setProperty("mute", false);
    if (volumePersistTimer) clearTimeout(volumePersistTimer);
    volumePersistTimer = setTimeout(() => void persistAudioSettings(), 400);
  }

  async function toggleMute() {
    await setProperty("mute", !isMuted.value);
    setTimeout(() => void persistAudioSettings(), 0);
  }

  // —— 倍速 ——
  async function setSpeed(s: number) {
    await setProperty("speed", s);
  }

  // —— 轨道切换（用 set 命令更可靠，禁用传 "no"）——
  async function setAudioTrack(id: number) {
    await command("set", ["aid", String(id)]);
  }

  async function setSubTrack(id: number) {
    // 0 = 禁用字幕，传 "no"；否则传轨道 id
    const val = id === 0 ? "no" : String(id);
    await command("set", ["sid", asSubValue(val)]);
  }

  // 帮助：mpv set 命令的 sid 值直接用字符串
  function asSubValue(v: string): string {
    return v;
  }

  // —— 截图：用 screenshot-to-file 指定路径，返回保存路径供前端提示 ——
  async function screenshot(includeSubtitles = true): Promise<string> {
    if (screenshotBusy.value) throw new Error("截图正在进行中");
    screenshotBusy.value = true;
    try {
      const dir = await invoke<string>("screenshots_dir");
      const fileName = buildScreenshotFileName(currentFileName.value || "screenshot", currentTime.value, includeSubtitles);
      const filePath = await join(dir, fileName);
      await command("screenshot-to-file", [filePath, includeSubtitles ? "subtitles" : "video"]);
      return filePath;
    } finally {
      screenshotBusy.value = false;
    }
  }

  // —— A-B 循环 ——
  async function setAbLoopA() {
    if (abLoopBusy.value) return;
    abLoopBusy.value = true;
    const oldA = abLoopA.value;
    const oldB = abLoopB.value;
    try {
      const t = currentTime.value;
      await setProperty("ab-loop-a", t);
      if (oldB != null && t >= oldB) {
        await setProperty("ab-loop-b", "no");
        abLoopB.value = null;
      }
      abLoopA.value = t;
    } catch (error) {
      abLoopA.value = oldA;
      abLoopB.value = oldB;
      throw error;
    } finally { abLoopBusy.value = false; }
  }
  async function setAbLoopB() {
    if (abLoopA.value == null) throw new Error("请先设置 A 点");
    const t = currentTime.value;
    if (t - abLoopA.value < 0.1) throw new Error("B 点必须晚于 A 点至少 0.1 秒");
    if (abLoopBusy.value) return;
    abLoopBusy.value = true;
    try {
      await setProperty("ab-loop-b", t);
      abLoopB.value = t;
      await command("seek", [abLoopA.value, "absolute"]);
    } finally { abLoopBusy.value = false; }
  }
  async function clearAbLoop() {
    if (abLoopBusy.value) return;
    abLoopBusy.value = true;
    const oldA = abLoopA.value;
    const oldB = abLoopB.value;
    try {
      await setProperty("ab-loop-a", "no");
      await setProperty("ab-loop-b", "no");
      abLoopA.value = null;
      abLoopB.value = null;
    } catch (error) {
      abLoopA.value = oldA;
      abLoopB.value = oldB;
      throw error;
    } finally { abLoopBusy.value = false; }
  }
  async function seekToAbLoopA() {
    if (abLoopA.value != null) await command("seek", [abLoopA.value, "absolute"]);
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

  // 翻转/旋转。
  //
  // 旋转用 video-rotate 属性（渲染层操作，硬解下也有效）。
  // 翻转用 vf 滤镜——但硬解(hwdec)下软件滤镜会被静默忽略（硬解画面留在 GPU，
  // 软件滤镜碰不到）。mpv 在 gpu-next 下不会为此自动回退软解。
  //
  // 解决：翻转时临时关闭硬解（hwdec=no），让 vf 滤镜生效；关闭翻转时恢复硬解。
  // 代价：翻转期间 CPU 占用升高（软解），但能保证翻转真正生效。
  // 关闭硬解后 mpv 会自动用软解，vf 滤镜随之生效。
  async function setFlipState(nextH: boolean, nextV: boolean) {
    if (transformBusy.value) return;
    transformBusy.value = true;
    const oldH = hFlipped.value;
    const oldV = vFlipped.value;
    const oldVf = await getProperty<string>("vf", "string").catch(() => "");
    const oldHwdec = await getProperty<string>("hwdec", "string").catch(() => "auto-safe");
    const chain = [nextH && "hflip", nextV && "vflip"].filter(Boolean).join(",");
    try {
      if (chain) {
        if (!oldH && !oldV) hwdecBeforeFlip = oldHwdec || "auto-safe";
        await setProperty("hwdec", "no");
        await setProperty("vf", chain);
      } else {
        await setProperty("vf", "");
        await setProperty("hwdec", hwdecBeforeFlip);
      }
      hFlipped.value = nextH;
      vFlipped.value = nextV;
    } catch (error) {
      await setProperty("vf", oldVf || "").catch(() => {});
      await setProperty("hwdec", oldHwdec || "auto-safe").catch(() => {});
      hFlipped.value = oldH;
      vFlipped.value = oldV;
      throw error;
    } finally {
      transformBusy.value = false;
    }
  }
  async function toggleHFlip() { await setFlipState(!hFlipped.value, vFlipped.value); }
  async function toggleVFlip() { await setFlipState(hFlipped.value, !vFlipped.value); }
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
    if (hFlipped.value || vFlipped.value) {
      await setFlipState(false, false);
    }
  }

  // 定时保存进度（每 5 秒）
  let progressTimer: ReturnType<typeof setInterval> | null = null;

  onMounted(async () => {
    try {
      await initMpv();
      await loadSettings();
      settingsReady.value = true;
      await setProperty("volume", volume.value);
      await setProperty("mute", isMuted.value);
    } catch (e) {
      playbackState.value = "failed";
      playbackError.value = e instanceof Error ? e.message : String(e);
      console.error("mpv init failed:", e);
    }
    progressTimer = setInterval(() => void saveProgress(), 5000);
  });

  onUnmounted(() => {
    loadGate.cancel();
    unlisten?.();
    unlistenEvents?.();
    if (progressTimer) clearInterval(progressTimer);
    if (volumePersistTimer) clearTimeout(volumePersistTimer);
    stopResizePoll();
    void saveProgress().finally(() => destroy().catch(() => {}));
  });

  // 快进/快退时间（秒），可通过设置面板修改
  const skipSeconds = ref(10);

  // 窗口缩放系数：用户可调，打开视频时窗口在原寸基础上乘以此系数；超出屏幕仍自动最大化
  const windowScale = ref(1);

  // 新视频播放起点："start"=从头、"resume"=从上次位置
  const resumeMode = ref<"start" | "resume">("resume");

  // —— 设置持久化：写入 SQLite settings 表 ——
  async function persistSetting(key: string, value: string) {
    try {
      await invoke("set_setting", { key, value });
    } catch (e) {
      console.warn(`[persistSetting] ${key} 失败:`, e);
    }
  }

  // 三个设置项的持久化 setter（改值 + 写库）
  async function setSkipSeconds(v: number) {
    skipSeconds.value = v;
    await persistSetting("skip_seconds", String(v));
  }
  async function setWindowScale(v: number) {
    windowScale.value = v;
    await persistSetting("window_scale", String(v));
  }
  async function setWindowSizePolicy(policy: WindowSizePolicy) {
    windowSizePolicy.value = policy;
    await persistSetting("window_size_policy", policy);
  }
  async function setResumeMode(m: "start" | "resume") {
    resumeMode.value = m;
    await persistSetting("resume_mode", m);
  }

  // 启动时从数据库加载设置（覆盖默认值）
  async function loadSettings() {
    const read = async <T>(key: string, parse: (s: string) => T | null): Promise<T | null> => {
      try {
        const v = await invoke<string | null>("get_setting", { key });
        if (v == null) return null;
        return parse(v);
      } catch {
        return null;
      }
    };
    const skip = await read("skip_seconds", (s) => {
      const n = parseInt(s, 10);
      return Number.isFinite(n) && n > 0 ? n : null;
    });
    if (skip) skipSeconds.value = skip;
    const scale = await read("window_scale", (s) => {
      const n = parseFloat(s);
      return Number.isFinite(n) && n > 0 ? n : null;
    });
    if (scale) windowScale.value = scale;
    const mode = await read<"start" | "resume">("resume_mode", (s) =>
      s === "start" || s === "resume" ? s : null
    );
    if (mode) resumeMode.value = mode;
    const policy = await read<WindowSizePolicy>("window_size_policy", (s) =>
      ["video", "keep", "fit", "maximize"].includes(s) ? s as WindowSizePolicy : null
    );
    if (policy) windowSizePolicy.value = policy;
    const storedVolume = await read("volume", (s) => {
      const n = Number(s); return Number.isFinite(n) && n >= 0 && n <= 100 ? n : null;
    });
    if (storedVolume != null) volume.value = storedVolume;
    const storedMuted = await read("muted", (s) => s === "true" ? true : s === "false" ? false : null);
    if (storedMuted != null) isMuted.value = storedMuted;
  }

  // 关闭当前文件
  async function closeFile() {
    await saveProgress();
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
    playbackState.value = "idle";
    playbackError.value = "";
  }

  return {
    // 状态
    isReady, settingsReady, isPlaying, currentTime, duration, volume, isMuted,
    playbackState, playbackError, pausedForCache, cacheBufferingState, bufferedUntil,
    currentFile, currentFileName, speed, videoHash, isOpening,
    audioTracks, subTracks, currentAudioId, currentSubId,
    aspectRatio, abLoopA, abLoopB, abLoopBusy, skipSeconds, windowScale, windowSizePolicy, resumeMode,
    screenshotBusy, transformBusy,
    videoWidth, videoHeight, videoRotate, hFlipped, vFlipped,
    // 文件
    openFileDialog, openFile, closeFile, loadSubtitle, loadSubtitleDialog, openDroppedFile,
    // 播放
    togglePlay, replay, seekTo, seekBy, setVolume, toggleMute, setSpeed, saveProgress,
    // 设置持久化
    setSkipSeconds, setWindowScale, setWindowSizePolicy, setResumeMode, loadSettings,
    // 轨道
    setAudioTrack, setSubTrack, refreshTracks,
    // 截图
    screenshot,
    // AB循环/逐帧
    setAbLoopA, setAbLoopB, clearAbLoop, seekToAbLoopA, frameBackStep, frameStep,
    // 画面
    setAspectRatio, toggleHFlip, toggleVFlip, rotate90, rotateMinus90, resetTransform,
  };
}
