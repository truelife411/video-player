<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { useMpv } from "./composables/useMpv";
import { useTags } from "./composables/useTags";
import { useSearch } from "./composables/useSearch";
import ControlBar from "./components/ControlBar.vue";
import SettingsPanel from "./components/SettingsPanel.vue";
import PlaybackPanel from "./components/PlaybackPanel.vue";
import TagCard from "./components/TagCard.vue";
import SearchOverlay from "./components/SearchOverlay.vue";

const mpv = useMpv();
const tags = useTags();
const search = useSearch();

// —— 浮层状态 ——
const showSettings = ref(false);
const showPlayback = ref(false);
const showTagCard = ref(false);
const showSearch = ref(false);

// 任何浮层打开时，禁用 surface 的点击（防止穿透到视频）
const anyOverlayOpen = computed(
  () => showSettings.value || showPlayback.value || showTagCard.value || showSearch.value
);
// 互斥：打开一个底部浮层时关闭另一个，避免重叠
function closeBottomPanels() {
  showSettings.value = false;
  showPlayback.value = false;
}

function toggleSettings() {
  const willOpen = !showSettings.value;
  closeBottomPanels();
  if (willOpen) {
    // 首次打开设置时加载标签类型，并确保预设标签存在（修复误删）
    if (tags.tagTypes.value.length === 0) tags.loadTagTypes();
    tags.ensurePresets().then(() => {
      if (tags.tagTypes.value.length === 0) tags.loadTagTypes();
    });
    showSettings.value = true;
  }
}

// 播放操作浮层（无视频时按钮已禁用，这里再保险一次）
function togglePlayback() {
  if (!mpv.currentFile.value) return;
  const willOpen = !showPlayback.value;
  closeBottomPanels();
  showPlayback.value = willOpen;
}

// —— 标签卡片（T 键或右键唤出）——
const videoHash = ref<string>("");
const tagCardLoading = ref(false);
async function toggleTagCard() {
  if (!mpv.currentFile.value) {
    showToast("请先打开视频");
    return;
  }
  if (tagCardLoading.value) return;
  if (showTagCard.value) {
    showTagCard.value = false;
    return;
  }
  tagCardLoading.value = true;
  try {
    // 复用 openFile 时已算的 hash，避免重复计算（大文件 hash 是主要耗时）
    let h = mpv.videoHash.value;
    if (!h) {
      h = await invoke<string>("register_video", { path: mpv.currentFile.value });
    }
    videoHash.value = h;
    // 标签类型只在首次或为空时加载（缓存，避免每次 T 键都查库）
    if (tags.tagTypes.value.length === 0) {
      await tags.loadTagTypes();
    }
    await tags.loadVideoTags(h);
    showTagCard.value = true;
  } catch (e) {
    console.error("[标签] 加载失败:", e);
    showToast("无法加载标签");
  } finally {
    tagCardLoading.value = false;
  }
}

// —— 启动/双击文件打开（文件关联的「最后一公里」）——
// 后端两条路径：① 首启从 argv 取（get_startup_file，取出即失效）；
// ② 已运行实例由 single-instance 插件 emit「open-file-argv」转发。
// 两路都要等 mpv 初始化完成（isReady）才能 openFile，否则 load 会失败。
async function openFileWhenReady(path: string) {
  if (!path) return;
  // 轮询等待 isReady（比 watch 稳：避免 watch 错过值变化导致永久不触发）。
  // 最多等 15 秒（mpv 初始化偶尔较慢），超时则放弃但仍尝试 openFile（best-effort）。
  const deadline = Date.now() + 15000;
  while (!mpv.isReady.value && Date.now() < deadline) {
    await new Promise((r) => setTimeout(r, 100));
  }
  try {
    await mpv.openFile(path);
  } catch (e) {
    console.error("[openFileWhenReady] 失败:", e);
    // openFile 失败也要把窗口显示出来，避免只剩声音没有界面
    try {
      const w = getCurrentWindow();
      await w.show();
      await w.setFocus();
    } catch {
      /* 忽略 */
    }
  }
}

// —— Toast 提示（截图等操作反馈）——
const toastMsg = ref("");
let toastTimer: number | null = null;
function showToast(msg: string) {
  toastMsg.value = msg;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => (toastMsg.value = ""), 2500);
}

// —— 搜索浮层（Ctrl+F）——
function toggleSearch() {
  showSearch.value = !showSearch.value;
  if (!showSearch.value) search.clear();
}
async function searchPlay(path: string) {
  await mpv.openFile(path);
}
async function searchReveal(path: string) {
  try {
    await invoke("reveal_in_explorer", { path });
  } catch (e) {
    console.error("[定位] 失败:", e);
    showToast("无法定位文件");
  }
}

// 截图并提示保存路径
async function takeScreenshot(withSubs: boolean) {
  try {
    const path = await mpv.screenshot(withSubs);
    showToast(`截图已保存：${path}`);
  } catch (e) {
    console.error("[截图] 失败:", e);
    showToast("截图失败");
  }
}

// —— 窗口 / 全屏 ——
const appWindow = getCurrentWindow();
const isFullscreen = ref(false);

// 监听窗口全屏状态变化（F11 或系统快捷键也能同步）
let unlistenFullscreen: UnlistenFn | null = null;

async function toggleFullscreen() {
  try {
    const next = !isFullscreen.value;
    await appWindow.setFullscreen(next);
    isFullscreen.value = next;
  } catch (e) {
    console.error("[全屏] 失败:", e);
  }
}

// —— 控制栏自动隐藏 ——
const controlsVisible = ref(true);
let hideTimer: number | null = null;

function showControls() {
  controlsVisible.value = true;
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = window.setTimeout(() => {
    if (mpv.isPlaying.value) controlsVisible.value = false;
  }, 3000);
}

function onMouseMove() {
  showControls();
}

// —— 单击/双击画面处理 ——
// 单击=播放/暂停，双击=全屏。用延时区分，避免单击触发后双击又触发。
let clickTimer: number | null = null;
function onSurfaceClick() {
  if (anyOverlayOpen.value) return; // 浮层打开时忽略画面点击
  if (clickTimer) return;
  clickTimer = window.setTimeout(() => {
    clickTimer = null;
    mpv.togglePlay();
    showControls();
  }, 220);
}

function onSurfaceDblClick() {
  if (clickTimer) {
    clearTimeout(clickTimer);
    clickTimer = null;
  }
  toggleFullscreen();
}

// —— 拖拽文件（Tauri 2 webview 拖拽事件，非浏览器 drop）——
let unlistenDrag: UnlistenFn | null = null;
// single-instance 转发的「双击文件打开」事件
let unlistenOpenFileArgv: UnlistenFn | null = null;

// —— 音量反馈：键盘调音量时短暂弹出滑块 ——
const volumeFlash = ref(0);
function flashVolume() {
  volumeFlash.value++;
}

// —— 快捷键 ——
function onKeyDown(e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  if (target.tagName === "INPUT" || target.tagName === "TEXTAREA") return;

  switch (e.code) {
    case "Space":
      e.preventDefault();
      mpv.togglePlay();
      showControls();
      break;
    case "ArrowRight":
      e.preventDefault();
      mpv.seekBy(mpv.skipSeconds.value);
      showControls();
      break;
    case "ArrowLeft":
      e.preventDefault();
      mpv.seekBy(-mpv.skipSeconds.value);
      showControls();
      break;
    case "ArrowUp":
      e.preventDefault();
      mpv.setVolume(Math.min(100, mpv.volume.value + 5));
      flashVolume();
      showControls();
      break;
    case "ArrowDown":
      e.preventDefault();
      mpv.setVolume(Math.max(0, mpv.volume.value - 5));
      flashVolume();
      showControls();
      break;
    case "KeyM":
      mpv.toggleMute();
      flashVolume();
      break;
    case "KeyF":
      // Ctrl+F = 搜索，单独 F = 全屏
      if (e.ctrlKey || e.metaKey) {
        e.preventDefault();
        toggleSearch();
      } else {
        toggleFullscreen();
      }
      break;
    case "KeyT":
      toggleTagCard();
      break;
    case "KeyS":
      // 截图（默认含字幕）
      if (mpv.currentFile.value) takeScreenshot(true);
      break;
    case "KeyC":
      // 切换字幕开关：当前有字幕则禁用，否则启用第一个
      if (mpv.currentSubId.value !== 0) {
        mpv.setSubTrack(0);
      } else if (mpv.subTracks.value.length > 0) {
        mpv.setSubTrack(mpv.subTracks.value[0].id);
      }
      break;
    case "Comma":
      // 逐帧后退
      if (mpv.currentFile.value) mpv.frameBackStep();
      break;
    case "Period":
      // 逐帧前进
      if (mpv.currentFile.value) mpv.frameStep();
      break;
    case "KeyR":
      // 画面旋转：R 顺时针 90°，Shift+R 逆时针 90°
      if (mpv.currentFile.value) {
        if (e.shiftKey) mpv.rotateMinus90();
        else mpv.rotate90();
        showControls();
      }
      break;
    case "KeyH":
      // 水平翻转
      if (mpv.currentFile.value) {
        mpv.toggleHFlip();
        showControls();
      }
      break;
    case "KeyV":
      // 垂直翻转（注意：避开与全屏 F 的混淆，V 专用于垂直翻转）
      if (mpv.currentFile.value) {
        mpv.toggleVFlip();
        showControls();
      }
      break;
  }
}

// —— 顶部文件名 ——
const displayName = computed(() => {
  if (mpv.currentFileName.value) return mpv.currentFileName.value;
  if (mpv.currentFile.value) return mpv.currentFile.value.split(/[\\/]/).pop() || "";
  return "";
});

onMounted(async () => {
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("mousemove", onMouseMove);
  showControls();

  // 启动时确保预设标签存在（修复误删）
  tags.ensurePresets();

  // 监听全屏状态（兼容系统快捷键）
  unlistenFullscreen = await appWindow.onResized(() => {
    appWindow.isFullscreen().then((f) => (isFullscreen.value = f));
  });

  // Tauri 2 拖拽：用 webview 的 onDragDropEvent
  const webview = getCurrentWebview();
  unlistenDrag = await webview.onDragDropEvent(async (event) => {
    if (event.payload.type === "drop") {
      const paths = event.payload.paths;
      if (paths && paths.length > 0) {
        await mpv.openDroppedFile(paths[0]);
      }
    }
  });

  // 首启：取出 argv 里的视频路径（双击文件且无已运行实例时走这里）
  let hadStartup = false;
  try {
    const startup = await invoke<string | null>("get_startup_file");
    if (startup) {
      hadStartup = true;
      await openFileWhenReady(startup);
    }
  } catch (e) {
    console.warn("[启动文件] 读取失败:", e);
  }

  // 无启动视频时直接显示窗口（openFile 内部也会 show，这里兜底纯启动场景）。
  // 有启动视频时由 openFile 的 resize 流程负责 show，避免提前露出默认尺寸。
  if (!hadStartup) {
    try {
      await appWindow.show();
      await appWindow.setFocus();
    } catch {
      /* 忽略 */
    }
  }

  // 已运行实例：single-instance 插件转发新双击的文件
  unlistenOpenFileArgv = await listen<string>("open-file-argv", (event) => {
    if (event.payload) openFileWhenReady(event.payload);
  });
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeyDown);
  window.removeEventListener("mousemove", onMouseMove);
  if (hideTimer) clearTimeout(hideTimer);
  if (clickTimer) clearTimeout(clickTimer);
  unlistenFullscreen?.();
  unlistenDrag?.();
  unlistenOpenFileArgv?.();
});
</script>

<template>
  <!-- 视频表面：透明，单击=播放/暂停，双击=全屏，右键=标签卡片 -->
  <div
    class="surface"
    :class="{
      'cursor-hidden': !controlsVisible && mpv.isPlaying.value,
      'overlay-open': anyOverlayOpen,
    }"
    @click="onSurfaceClick"
    @dblclick="onSurfaceDblClick"
    @contextmenu.prevent="toggleTagCard"
  >
    <!-- 顶部：文件名（控件层，自动隐藏） -->
    <Transition name="fade">
      <div v-show="controlsVisible && displayName" class="overlay-top" @click.stop>
        <span class="badge" v-if="!mpv.isReady.value">初始化中…</span>
        <span class="filename">{{ displayName }}</span>
      </div>
    </Transition>

    <!-- 标签卡片（右上角浮层） -->
    <Transition name="pop">
      <TagCard
        v-if="showTagCard"
        class="tagcard-pos"
        :tag-types="tags.tagTypes.value"
        :get-value="tags.getValue"
        :hash="videoHash"
        @close="showTagCard = false"
        @set-value="(id, v) => tags.setValue(id, v)"
      />
    </Transition>

    <!-- 中央占位（未打开文件时的提示，打开文件后优雅淡出） -->
    <Transition name="hint-fade">
      <div v-if="mpv.isReady.value && !mpv.currentFile.value" class="empty-hint" @click.stop>
        <div class="empty-icon">🎬</div>
        <p>拖拽视频文件到此处，或点击下方"打开文件"</p>
      </div>
    </Transition>

    <!-- 打开视频中：掩盖窗口尺寸调整（兜底路径隐藏窗口时，显示前不会看到此层） -->
    <Transition name="fade">
      <div v-if="mpv.isOpening.value" class="opening-hint" @click.stop>
        <div class="spinner"></div>
        <p>正在打开…</p>
      </div>
    </Transition>

    <!-- 底部：控制栏（自动隐藏） -->
    <Transition name="slide-up">
      <div v-show="controlsVisible" class="bottom-wrap">
        <!-- 设置面板（浮在控制栏右上） -->
        <Transition name="pop">
          <SettingsPanel
            v-if="showSettings"
            class="settings-pos"
            :skip-seconds="mpv.skipSeconds.value"
            :tag-types="tags.tagTypes.value"
            :window-scale="mpv.windowScale.value"
            :resume-mode="mpv.resumeMode.value"
            @close="showSettings = false"
            @set-skip-seconds="(s) => mpv.setSkipSeconds(s)"
            @set-window-scale="(s) => mpv.setWindowScale(s)"
            @set-resume-mode="(m) => mpv.setResumeMode(m)"
            @create-tag-type="(name, vt, opts) => tags.createTagType(name, vt, opts)"
            @delete-tag-type="(id) => tags.deleteTagType(id)"
          />
        </Transition>

        <!-- 播放操作面板（浮在控制栏右上） -->
        <Transition name="pop">
          <PlaybackPanel
            v-if="showPlayback"
            class="settings-pos"
            :speed="mpv.speed.value"
            :audio-tracks="mpv.audioTracks.value"
            :sub-tracks="mpv.subTracks.value"
            :current-audio-id="mpv.currentAudioId.value"
            :current-sub-id="mpv.currentSubId.value"
            :ab-loop-a="mpv.abLoopA.value"
            :ab-loop-b="mpv.abLoopB.value"
            :video-rotate="mpv.videoRotate.value"
            :h-flipped="mpv.hFlipped.value"
            :v-flipped="mpv.vFlipped.value"
            @close="showPlayback = false"
            @set-speed="(s) => mpv.setSpeed(s)"
            @set-audio="(id) => mpv.setAudioTrack(id)"
            @set-sub="(id) => mpv.setSubTrack(id)"
            @load-subtitle="mpv.loadSubtitleDialog()"
            @screenshot="(w) => takeScreenshot(w)"
            @set-ab-a="mpv.setAbLoopA()"
            @set-ab-b="mpv.setAbLoopB()"
            @clear-ab="mpv.clearAbLoop()"
            @frame-back="mpv.frameBackStep()"
            @frame-forward="mpv.frameStep()"
            @rotate-cw="mpv.rotate90()"
            @rotate-ccw="mpv.rotateMinus90()"
            @toggle-h-flip="mpv.toggleHFlip()"
            @toggle-v-flip="mpv.toggleVFlip()"
            @reset-transform="mpv.resetTransform()"
          />
        </Transition>

        <ControlBar
          :is-playing="mpv.isPlaying.value"
          :current-time="mpv.currentTime.value"
          :duration="mpv.duration.value"
          :volume="mpv.volume.value"
          :is-muted="mpv.isMuted.value"
          :has-file="!!mpv.currentFile.value"
          :is-fullscreen="isFullscreen"
          :force-show-volume="volumeFlash"
          :skip-seconds="mpv.skipSeconds.value"
          @toggle-play="mpv.togglePlay()"
          @seek="(s) => mpv.seekTo(s)"
          @set-volume="(v) => mpv.setVolume(v)"
          @toggle-mute="mpv.toggleMute()"
          @toggle-fullscreen="toggleFullscreen"
          @open-file="mpv.openFileDialog()"
          @close-file="mpv.closeFile()"
          @toggle-settings="toggleSettings"
          @toggle-playback="togglePlayback"
          @toggle-tag-card="toggleTagCard"
          @rotate-cw="mpv.rotate90()"
          @rotate-ccw="mpv.rotateMinus90()"
        />
      </div>
    </Transition>

    <!-- Toast 提示（截图等反馈） -->
    <Transition name="toast">
      <div v-if="toastMsg" class="toast" @click.stop>{{ toastMsg }}</div>
    </Transition>

    <!-- 搜索浮层（Ctrl+F） -->
    <Transition name="fade">
      <SearchOverlay
        v-if="showSearch"
        :keyword="search.keyword.value"
        :results="search.results.value"
        :searching="search.searching.value"
        @close="showSearch = false"
        @updatekeyword="(k: string) => search.setKeyword(k)"
        @play="(p: string) => searchPlay(p)"
        @reveal="(p: string) => searchReveal(p)"
      />
    </Transition>
  </div>
</template>

<style scoped>
.surface {
  position: fixed;
  inset: 0;
  background: transparent;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  cursor: default;
  user-select: none;
}

/* 浮层打开时：禁用 surface 点击，防止穿透到视频；
   浮层和控制栏自身重新启用 pointer-events */
.surface.overlay-open {
  pointer-events: none;
}
.surface.overlay-open :where(.tagcard-pos, .bottom-wrap, .overlay-top) {
  pointer-events: auto;
}

/* 底部容器：控制栏 + 浮在上方的设置面板 */
.bottom-wrap {
  position: relative;
}

.settings-pos {
  position: absolute;
  right: 16px;
  bottom: calc(100% + 8px);
}

.surface.cursor-hidden {
  cursor: none;
}

/* 顶部文件名条 */
.overlay-top {
  flex-shrink: 0;
  padding: 16px 20px;
  display: flex;
  align-items: center;
  gap: 12px;
  background: linear-gradient(180deg, rgba(0, 0, 0, 0.55), transparent);
}

.badge {
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 13px;
  background: rgba(255, 255, 255, 0.15);
  backdrop-filter: blur(8px);
}

.badge.hint {
  margin-left: auto;
  font-size: 11px;
  cursor: pointer;
}
.badge.hint:hover {
  background: rgba(255, 255, 255, 0.25);
}

/* 标签卡片：右上角浮层，z-index 极高确保在视频之上 */
.tagcard-pos {
  position: absolute;
  top: 60px;
  right: 20px;
  z-index: 9999;
}

.filename {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.85);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 空状态提示：居中但用 flex 占位，不影响控制栏贴底 */
.empty-hint {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: rgba(255, 255, 255, 0.5);
  pointer-events: none;
}

.empty-icon {
  font-size: 56px;
  opacity: 0.6;
}

/* 打开视频中的加载提示（掩盖窗口调整） */
.opening-hint {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
  color: rgba(255, 255, 255, 0.7);
  background: rgba(0, 0, 0, 0.4);
  pointer-events: none;
}
.opening-hint p {
  font-size: 14px;
}
.spinner {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: 3px solid rgba(255, 255, 255, 0.2);
  border-top-color: #4ea1ff;
  animation: spin 0.8s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.empty-hint p {
  font-size: 14px;
}

/* 过渡动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: transform 0.3s ease, opacity 0.3s ease;
}
.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100%);
  opacity: 0;
}

/* 设置面板弹出过渡 */
.pop-enter-active,
.pop-leave-active {
  transition: transform 0.2s ease, opacity 0.2s ease;
}
.pop-enter-from,
.pop-leave-to {
  transform: translateY(12px) scale(0.96);
  opacity: 0;
}

/* Toast 提示 */
.toast {
  position: absolute;
  bottom: 90px;
  left: 50%;
  transform: translateX(-50%);
  max-width: 70%;
  padding: 10px 18px;
  border-radius: 10px;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.95);
  background: rgba(20, 20, 26, 0.9);
  backdrop-filter: blur(14px);
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.4);
  word-break: break-all;
  pointer-events: none;
  z-index: 50;
}

/* 空状态提示淡出（打开文件时优雅消失） */
.hint-fade-enter-active,
.hint-fade-leave-active {
  transition: opacity var(--dur-normal) var(--ease-out), transform var(--dur-normal) var(--ease-out);
}
.hint-fade-enter-from,
.hint-fade-leave-to {
  opacity: 0;
  transform: scale(0.95);
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.3s ease, transform 0.3s ease;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(8px);
}
</style>
