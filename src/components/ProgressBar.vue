<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { formatTime } from "../utils/format";

const props = defineProps<{
  current: number; // 当前时间（秒）
  duration: number; // 总时长（秒）
  disabled?: boolean;
}>();

const emit = defineEmits<{
  seek: [seconds: number];
}>();

// —— 拖拽状态 ——
const isDragging = ref(false);
const hoverRatio = ref<number | null>(null); // 鼠标悬停位置（用于预览）

// 显示用的时间：拖拽时显示拖拽位置，否则显示真实进度
const displayRatio = computed(() => {
  if (props.duration <= 0) return 0;
  const base = isDragging.value ? dragValue.value : props.current;
  return Math.min(1, Math.max(0, base / props.duration));
});

// 拖拽过程中的临时值
const dragValue = ref(0);
watch(
  () => props.current,
  () => {
    if (!isDragging.value) dragValue.value = props.current;
  }
);

// 从鼠标事件计算时间值
function ratioFromEvent(e: MouseEvent | PointerEvent, el: HTMLElement): number {
  const rect = el.getBoundingClientRect();
  const x = e.clientX - rect.left;
  return Math.min(1, Math.max(0, x / rect.width));
}

function onPointerDown(e: PointerEvent) {
  if (props.disabled || props.duration <= 0) return;
  const el = e.currentTarget as HTMLElement;
  const ratio = ratioFromEvent(e, el);
  dragValue.value = ratio * props.duration;
  isDragging.value = true;
  el.setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
  const el = e.currentTarget as HTMLElement;
  const ratio = ratioFromEvent(e, el);
  hoverRatio.value = ratio;
  if (isDragging.value) {
    dragValue.value = ratio * props.duration;
  }
}

function onPointerUp(e: PointerEvent) {
  if (!isDragging.value) return;
  const el = e.currentTarget as HTMLElement;
  const ratio = ratioFromEvent(e, el);
  const target = ratio * props.duration;
  isDragging.value = false;
  el.releasePointerCapture(e.pointerId);
  emit("seek", target);
}

function onPointerLeave() {
  hoverRatio.value = null;
}

// 预览气泡的时间
const hoverTime = computed(() => {
  if (hoverRatio.value === null || props.duration <= 0) return null;
  return hoverRatio.value * props.duration;
});

const displayTime = computed(() =>
  isDragging.value ? dragValue.value : props.current
);
</script>

<template>
  <div class="progress" :class="{ disabled }">
    <!-- 当前时间 -->
    <span class="time-label">{{ formatTime(displayTime) }}</span>

    <!-- 进度条轨道 -->
    <div
      class="track"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointerleave="onPointerLeave"
    >
      <!-- 悬停预览气泡 -->
      <div
        v-if="hoverTime !== null"
        class="hover-bubble"
        :style="{ left: (hoverRatio! * 100).toString() + '%' }"
      >
        {{ formatTime(hoverTime) }}
      </div>

      <!-- 缓冲层（暂用进度填充，后续接 mpv cache） -->
      <div class="buffered" :style="{ width: displayRatio * 100 + '%' }"></div>
      <!-- 已播放 -->
      <div class="played" :style="{ width: displayRatio * 100 + '%' }">
        <div class="thumb" :class="{ active: isDragging }"></div>
      </div>
    </div>

    <!-- 总时长 -->
    <span class="time-label">{{ formatTime(duration) }}</span>
  </div>
</template>

<style scoped>
.progress {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 4px 0;
}

.progress.disabled {
  opacity: 0.4;
  pointer-events: none;
}

.time-label {
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  color: rgba(255, 255, 255, 0.75);
  min-width: 42px;
  text-align: center;
  user-select: none;
}

/* 轨道 */
.track {
  position: relative;
  flex: 1;
  height: 18px;
  display: flex;
  align-items: center;
  cursor: pointer;
}

.track::before {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  height: 4px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.25);
  transition: height 0.15s ease;
}

.track:hover::before {
  height: 6px;
}

.buffered,
.played {
  position: absolute;
  left: 0;
  height: 4px;
  border-radius: 2px;
  pointer-events: none;
  transition: height 0.15s ease;
}

.buffered {
  background: rgba(255, 255, 255, 0.35);
  z-index: 1;
}

.played {
  background: linear-gradient(90deg, var(--color-accent), var(--color-accent-strong));
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: flex-end;
}

.track:hover .buffered,
.track:hover .played {
  height: 6px;
}

/* 拖拽手柄 */
.thumb {
  width: 13px;
  height: 13px;
  border-radius: 50%;
  background: #fff;
  margin-right: -6.5px;
  box-shadow: 0 0 0 0 rgba(255, 255, 255, 0);
  opacity: 0;
  transform: scale(0.8);
  transition: opacity 0.15s ease, transform 0.15s ease, box-shadow 0.2s ease;
}

.track:hover .thumb,
.thumb.active {
  opacity: 1;
  transform: scale(1);
}

.thumb.active {
  transform: scale(1.25);
  box-shadow: 0 0 12px rgba(78, 161, 255, 0.7);
}

/* 悬停预览气泡 */
.hover-bubble {
  position: absolute;
  bottom: 22px;
  transform: translateX(-50%);
  padding: 3px 8px;
  border-radius: 5px;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  color: #fff;
  background: rgba(0, 0, 0, 0.8);
  backdrop-filter: blur(8px);
  pointer-events: none;
  white-space: nowrap;
}
</style>
