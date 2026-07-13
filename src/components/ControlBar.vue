<script setup lang="ts">
import { ref, computed, watch } from "vue";
import ProgressBar from "./ProgressBar.vue";

const props = defineProps<{
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  volume: number;
  isMuted: boolean;
  hasFile: boolean;
  isFullscreen: boolean;
  // 外部（如快捷键）要求短暂显示音量滑块：用递增计数器触发 watch
  forceShowVolume?: number;
  skipSeconds?: number;
}>();

const emit = defineEmits<{
  togglePlay: [];
  seek: [seconds: number];
  setVolume: [v: number];
  toggleMute: [];
  toggleFullscreen: [];
  openFile: [];
  closeFile: [];
  toggleSettings: [];
  togglePlayback: [];
  toggleTagCard: [];
  toggleSearch: [];
  rotateCw: [];
  rotateCcw: [];
}>();

// —— 音量滑块显隐 ——
// hover 音量区域时显示；或外部 forceShowVolume（键盘调音量）时短暂显示
const hovering = ref(false);
const showVolumeSlider = computed(() => hovering.value || flashVolume.value);

// 键盘触发时短暂闪现
const flashVolume = ref(false);
let flashTimer: number | null = null;
watch(
  () => props.forceShowVolume,
  (v) => {
    if (!v) return;
    flashVolume.value = true;
    if (flashTimer) clearTimeout(flashTimer);
    flashTimer = window.setTimeout(() => (flashVolume.value = false), 1200);
  }
);

const volumePercent = computed(() => (props.isMuted ? 0 : props.volume));

function onVolumeWheel(e: WheelEvent) {
  e.preventDefault();
  const delta = e.deltaY < 0 ? 5 : -5;
  emit("setVolume", Math.min(100, Math.max(0, props.volume + delta)));
}

// 拖拽 range 时持续显示
const draggingVolume = ref(false);

const volumeIcon = computed(() => {
  if (props.isMuted || props.volume === 0) return "🔇";
  if (props.volume < 33) return "🔈";
  if (props.volume < 66) return "🔉";
  return "🔊";
});
</script>

<template>
  <div class="control-bar" @click.stop @wheel.passive>
    <!-- 顶行：进度条 -->
    <ProgressBar
      :current="currentTime"
      :duration="duration"
      :disabled="!hasFile"
      @seek="(s) => emit('seek', s)"
    />

    <!-- 下行：按钮组 -->
    <div class="buttons">
      <!-- 左侧 -->
      <div class="group">
        <!-- 打开文件：方框 + 蓝底 + 白色文件夹 -->
        <button
          class="icon-btn ctl"
          title="打开文件"
          @click="emit('openFile')"
        >
          <span class="ctl-inner"><i class="icon-folder"></i></span>
        </button>

        <!-- 播放/暂停：圆形 + 蓝底 + 白色三角/竖条 -->
        <button
          class="icon-btn play"
          :disabled="!hasFile"
          :title="isPlaying ? '暂停 (空格)' : '播放 (空格)'"
          @click="emit('togglePlay')"
        >
          <Transition name="icon-pop" mode="out-in">
            <span v-if="isPlaying" key="pause" class="play-icon icon-pause"></span>
            <span v-else key="play" class="play-icon icon-play"></span>
          </Transition>
        </button>

        <!-- 关闭：方框 + 蓝底 + 白色方块（位于播放与快退之间） -->
        <button
          class="icon-btn ctl"
          :disabled="!hasFile"
          title="关闭当前视频"
          @click="emit('closeFile')"
        >
          <span class="ctl-inner"><i class="icon-close-square"></i></span>
        </button>

        <!-- 快退：方框 + 蓝底 + 白色双左三角 -->
        <button
          class="icon-btn ctl"
          :disabled="!hasFile"
          :title="`后退 ${skipSeconds || 10} 秒 (←)`"
          @click="emit('seek', Math.max(0, currentTime - (skipSeconds || 10)))"
        >
          <span class="ctl-inner"><i class="icon-back"></i></span>
        </button>

        <!-- 快进：方框 + 蓝底 + 白色双右三角 -->
        <button
          class="icon-btn ctl"
          :disabled="!hasFile"
          :title="`前进 ${skipSeconds || 10} 秒 (→)`"
          @click="emit('seek', Math.min(duration, currentTime + (skipSeconds || 10)))"
        >
          <span class="ctl-inner"><i class="icon-fwd"></i></span>
        </button>
      </div>

      <!-- 右侧 -->
      <div class="group">
        <!-- 搜索（放大镜，与标签按钮风格一致） -->
        <button
          class="icon-btn glyph"
          title="搜索 (Ctrl+F)"
          @click="emit('toggleSearch')"
        >
          <svg viewBox="0 0 24 24" class="g"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>
        </button>

        <!-- 标签 -->
        <button
          class="icon-btn glyph"
          :disabled="!hasFile"
          title="标签 (T)"
          @click="emit('toggleTagCard')"
        >
          <svg viewBox="0 0 24 24" class="g"><path d="M20.5 13.5 13 21l-9-9V4h8l8.5 9.5ZM7.5 8.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z"/></svg>
        </button>

        <!-- 播放操作：无视频时灰色禁用 -->
        <button
          class="icon-btn glyph"
          :disabled="!hasFile"
          title="播放操作"
          @click="emit('togglePlayback')"
        >
          <svg viewBox="0 0 24 24" class="g"><circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="2.2" fill="currentColor" stroke="none"/><circle cx="12" cy="5" r="1.3" fill="currentColor" stroke="none"/><circle cx="19" cy="12" r="1.3" fill="currentColor" stroke="none"/><circle cx="12" cy="19" r="1.3" fill="currentColor" stroke="none"/><circle cx="5" cy="12" r="1.3" fill="currentColor" stroke="none"/></svg>
        </button>

        <!-- 音量：整个区域 hover 都保持显示，含滑块本体 -->
        <div
          class="volume-wrap"
          @mouseenter="hovering = true"
          @mouseleave="hovering = false"
          @wheel="onVolumeWheel"
        >
          <button class="icon-btn" title="静音 (M)" @click="emit('toggleMute')">
            {{ volumeIcon }}
          </button>
          <div class="volume-slider" :class="{ show: showVolumeSlider }">
            <input
              type="range"
              min="0"
              max="100"
              step="1"
              :value="volumePercent"
              orient="vertical"
              @input="(e) => emit('setVolume', Number((e.target as HTMLInputElement).value))"
              @mousedown="draggingVolume = true"
              @mouseup="draggingVolume = false"
            />
            <span class="volume-num">{{ Math.round(volumePercent) }}</span>
          </div>
        </div>

        <!-- 逆时针旋转 90°（位于顺时针按钮左侧） -->
        <button
          class="icon-btn glyph"
          :disabled="!hasFile"
          title="逆时针旋转 90° (Shift+R)"
          @click.stop.prevent="emit('rotateCcw')"
        >
          <svg viewBox="0 0 24 24" class="g">
            <path d="M5.64 7.64a8 8 0 1 1-1.69 5.28"/>
            <path d="M5 5v4h4"/>
          </svg>
        </button>

        <!-- 顺时针旋转 90° -->
        <button
          class="icon-btn glyph"
          :disabled="!hasFile"
          title="顺时针旋转 90° (R)"
          @click.stop.prevent="emit('rotateCw')"
        >
          <svg viewBox="0 0 24 24" class="g">
            <path d="M18.36 7.64a8 8 0 1 0 1.69 5.28"/>
            <path d="M19 5v4h-4"/>
          </svg>
        </button>

        <!-- 设置：常亮，随时可点 -->
        <button class="icon-btn glyph" title="设置" @click="emit('toggleSettings')">
          <svg viewBox="0 0 24 24" class="g">
            <circle cx="12" cy="12" r="3.2"/>
            <path d="M19.4 13.5a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V20a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 8.94 18.3a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.65 8.94a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34H9a1.7 1.7 0 0 0 1-1.56V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V9a1.7 1.7 0 0 0 1.56 1H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.51 1.5Z"/>
          </svg>
        </button>

        <!-- 全屏 -->
        <button
          class="icon-btn"
          :title="isFullscreen ? '退出全屏 (F)' : '全屏 (F)'"
          @click="emit('toggleFullscreen')"
        >
          {{ isFullscreen ? "🗗" : "⛶" }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.control-bar {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 16px 14px;
  background: linear-gradient(0deg, rgba(0, 0, 0, 0.7) 0%, rgba(0, 0, 0, 0.3) 70%, transparent 100%);
}

.buttons {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.group {
  display: flex;
  align-items: center;
  gap: 4px;
}

.icon-btn {
  width: 36px;
  height: 36px;
  border-radius: 8px;
  font-size: 16px;
  color: rgba(255, 255, 255, 0.9);
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s ease, transform 0.1s ease;
}

.icon-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.15);
}

.icon-btn:active:not(:disabled) {
  transform: scale(0.92);
}

.icon-btn:disabled {
  opacity: 0.35;
  cursor: default;
}

.icon-btn.play {
  font-size: 18px;
  /* 播放按钮更醒目：圆形 + 强调色背景 */
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: var(--color-accent);
  color: #fff;
  box-shadow: 0 4px 14px rgba(78, 161, 255, 0.35);
}
.icon-btn.play:hover:not(:disabled) {
  background: var(--color-accent-strong);
  box-shadow: 0 6px 18px rgba(78, 161, 255, 0.5);
}

/* —— 关闭/快退/快进：统一"透明外圈 + 蓝色圆角方块 + 白色几何符号" ——
   外圈与普通图标按钮完全一致（36×36 透明），所以视觉上不会比其他按钮大 */
.icon-btn.ctl {
  background: transparent;
}
.icon-btn.ctl:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.15);
}

/* 内层蓝色圆角方块 */
.ctl-inner {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: 5px;
  background: var(--color-accent);
  transition: background var(--dur-fast) ease;
}
.icon-btn.ctl:hover:not(:disabled) .ctl-inner {
  background: var(--color-accent-strong);
}
.ctl-inner i {
  display: inline-block;
}

/* 关闭：白色实心小方块 */
.icon-close-square {
  width: 9px;
  height: 9px;
  background: #fff;
  border-radius: 1px;
}

/* 打开文件：白色文件夹 */
.icon-folder {
  width: 12px;
  height: 10px;
  position: relative;
}
.icon-folder::before {
  /* 文件夹主体（含底部、左右、上沿后部） */
  content: "";
  position: absolute;
  inset: 2px 0 0 0;
  border: 2px solid #fff;
  border-top: none;
  border-radius: 0 2px 2px 2px;
}
.icon-folder::after {
  /* 文件夹顶板（凸起的 tab，覆盖在主体上方） */
  content: "";
  position: absolute;
  top: 0;
  left: 0;
  width: 5px;
  height: 3px;
  border: 2px solid #fff;
  border-bottom: none;
  border-radius: 2px 2px 0 0;
}

/* —— 面板触发按钮（标签/播放操作/设置）：透明外圈 + 线条图标，统一风格 —— */
.icon-btn.glyph {
  background: transparent;
}
.icon-btn.glyph:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.15);
}
.icon-btn.glyph .g {
  width: 20px;
  height: 20px;
  fill: none;
  stroke: rgba(255, 255, 255, 0.9);
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
  /* 整个 SVG 区域都接收点击，避免点在图形空白处穿透 */
  pointer-events: all;
  transition: stroke var(--dur-fast) ease;
}
.icon-btn.glyph:hover:not(:disabled) .g {
  stroke: #fff;
}

/* 快退：白色双左三角 */
.icon-back {
  width: 11px;
  height: 10px;
  position: relative;
}
.icon-back::before,
.icon-back::after {
  content: "";
  position: absolute;
  top: 0;
  width: 0;
  height: 0;
  border-top: 5px solid transparent;
  border-bottom: 5px solid transparent;
}
.icon-back::before {
  /* 第一段（左指三角）*/
  left: 0;
  border-right: 6px solid #fff;
}
.icon-back::after {
  /* 第二段（左指三角，靠右）*/
  right: 0;
  border-right: 6px solid #fff;
}

/* 快进：白色双右三角 */
.icon-fwd {
  width: 11px;
  height: 10px;
  position: relative;
}
.icon-fwd::before,
.icon-fwd::after {
  content: "";
  position: absolute;
  top: 0;
  width: 0;
  height: 0;
  border-top: 5px solid transparent;
  border-bottom: 5px solid transparent;
}
.icon-fwd::before {
  left: 0;
  border-left: 6px solid #fff;
}
.icon-fwd::after {
  right: 0;
  border-left: 6px solid #fff;
}

.play-icon {
  display: inline-block;
  position: relative;
  width: 16px;
  height: 16px;
}

/* 播放：CSS 三角形（向右） */
.icon-play {
  width: 0;
  height: 0;
  border-left: 13px solid #fff;
  border-top: 8px solid transparent;
  border-bottom: 8px solid transparent;
  margin-left: 3px;
}

/* 暂停：两根竖条 */
.icon-pause::before,
.icon-pause::after {
  content: "";
  position: absolute;
  top: 1px;
  width: 4px;
  height: 14px;
  background: #fff;
  border-radius: 1px;
}
.icon-pause::before {
  left: 3px;
}
.icon-pause::after {
  right: 3px;
}

/* 播放/暂停图标切换动画 */
.icon-pop-enter-active,
.icon-pop-leave-active {
  transition: transform var(--dur-fast) var(--ease-spring), opacity var(--dur-fast) ease;
}
.icon-pop-enter-from {
  transform: scale(0.4);
  opacity: 0;
}
.icon-pop-leave-to {
  transform: scale(0.4);
  opacity: 0;
}

/* 音量区域：按钮 + 滑块容器，hover 整体保持显示 */
.volume-wrap {
  position: relative;
  display: flex;
  align-items: center;
  /* 向上延伸可 hover 区域，覆盖滑块 */
  padding-top: 12px;
  margin-top: -12px;
}

.volume-slider {
  position: absolute;
  bottom: 44px;
  left: 50%;
  transform: translateX(-50%) scaleY(0);
  transform-origin: bottom center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 12px 8px;
  border-radius: 10px;
  background: rgba(20, 20, 24, 0.9);
  backdrop-filter: blur(14px);
  opacity: 0;
  pointer-events: none;
  transition: transform 0.2s ease, opacity 0.2s ease;
}

.volume-slider.show {
  transform: translateX(-50%) scaleY(1);
  opacity: 1;
  pointer-events: auto;
}

/* 竖向 range */
.volume-slider input[type="range"] {
  -webkit-appearance: slider-vertical;
  writing-mode: bt-lr;
  width: 6px;
  height: 90px;
  cursor: pointer;
}

.volume-num {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.7);
  font-variant-numeric: tabular-nums;
}
</style>
