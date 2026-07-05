import { ref, onMounted, onUnmounted } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { pictureDir, join } from "@tauri-apps/api/path";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
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
  // AB 循环
  const abLoopA = ref<number | null>(null);
  const abLoopB = ref<number | null>(null);

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
    await command("loadfile", [path]);
    currentFile.value = path;
    // mpv 默认加载后自动播放，主动同步状态
    isPlaying.value = true;
    // 重置 AB 循环
    abLoopA.value = null;
    abLoopB.value = null;
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
              videoHash: h, // 注意这里 h 是 hash 变量
              typeId: qualityType.id,
              value: quality,
            });
          } catch (e) {
            console.warn("[自动标注画质] 失败:", e);
          }
        }, 1200);
        // 根据视频分辨率自动调整窗口大小
        setTimeout(async () => {
          try {
            const w = videoWidth.value;
            const vh2 = videoHeight.value;
            if (w <= 0 || vh2 <= 0) return;
            const screenW = window.screen.availWidth;
            const screenH = window.screen.availHeight;
            const maxW = Math.round(screenW * 0.9);
            const maxH = Math.round(screenH * 0.85);
            let targetW = w;
            let targetH = vh2;
            if (targetW > maxW || targetH > maxH) {
              const scale = Math.min(maxW / targetW, maxH / targetH);
              targetW = Math.round(targetW * scale);
              targetH = Math.round(targetH * scale);
            }
            const appWindow = getCurrentWindow();
            await appWindow.setSize(new LogicalSize(targetW, targetH));
          } catch (e) {
            console.warn("[自动调整窗口] 失败:", e);
          }
        }, 1500);
        try {
          const info = await invoke<{ play_position: number; duration: number } | null>(
            "get_video",
            { hash: h }
          );
          if (info && info.play_position > 5) {
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

  // 翻转/旋转
  async function toggleHFlip() {
    const cur = await getProperty<boolean>("vf", "flag").catch(() => false);
    await command("vf", ["toggle", "hflip"]);
    void cur;
  }
  async function toggleVFlip() {
    await command("vf", ["toggle", "vflip"]);
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
    // 退出时最后保存一次（不等待，best-effort）
    saveProgress();
  });

  // 快进/快退时间（秒），可通过设置面板修改
  const skipSeconds = ref(10);

  // 关闭当前文件
  async function closeFile() {
    await command("stop");
    currentFile.value = "";
    currentFileName.value = "";
    videoHash.value = "";
    isPlaying.value = false;
    currentTime.value = 0;
    duration.value = 0;
    abLoopA.value = null;
    abLoopB.value = null;
  }

  return {
    // 状态
    isReady, isPlaying, currentTime, duration, volume, isMuted,
    currentFile, currentFileName, speed, videoHash,
    audioTracks, subTracks, currentAudioId, currentSubId,
    aspectRatio, abLoopA, abLoopB, skipSeconds,
    videoWidth, videoHeight,
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
    setAspectRatio, toggleHFlip, toggleVFlip,
    setBrightness, setContrast, setSaturation,
  };
}
