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
  toggleTagCard: [];
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
        <button class="icon-btn" title="打开文件" @click="emit('openFile')">📁</button>
        <button
          class="icon-btn close"
          :disabled="!hasFile"
          title="关闭当前视频"
          @click="emit('closeFile')"
        >
          <span class="close-icon"></span>
        </button>
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
        <button
          class="icon-btn"
          :disabled="!hasFile"
          :title="`后退 ${skipSeconds || 10} 秒 (←)`"
          @click="emit('seek', Math.max(0, currentTime - (skipSeconds || 10)))"
        >
          ⏪
        </button>
        <button
          class="icon-btn"
          :disabled="!hasFile"
          :title="`前进 ${skipSeconds || 10} 秒 (→)`"
          @click="emit('seek', Math.min(duration, currentTime + (skipSeconds || 10)))"
        >
          ⏩
        </button>
      </div>

      <!-- 右侧 -->
      <div class="group">
        <!-- 标签 -->
        <button
          class="icon-btn"
          :disabled="!hasFile"
          title="标签 (T)"
          @click="emit('toggleTagCard')"
        >
          🏷
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

        <!-- 设置 -->
        <button
          class="icon-btn"
          :disabled="!hasFile"
          title="设置"
          @click="emit('toggleSettings')"
        >
          ⚙
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

/* 关闭按钮：蓝色底色 + 白色正方形，与快进快退类似 */
.icon-btn.close {
  background: var(--color-accent);
  box-shadow: 0 4px 14px rgba(78, 161, 255, 0.35);
}
.icon-btn.close:hover:not(:disabled) {
  background: var(--color-accent-strong);
  box-shadow: 0 6px 18px rgba(78, 161, 255, 0.5);
}

/* 关闭按钮：CSS 实心正方形 */
.close-icon {
  display: inline-block;
  width: 12px;
  height: 12px;
  background: #fff;
  border-radius: 2px;
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
