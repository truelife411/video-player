<script setup lang="ts">
import { SPEED_PRESETS, type Track } from "../composables/useMpv";

defineProps<{
  speed: number;
  audioTracks: Track[];
  subTracks: Track[];
  currentAudioId: number;
  currentSubId: number;
  abLoopA: number | null;
  abLoopB: number | null;
  videoRotate: number;
  hFlipped: boolean;
  vFlipped: boolean;
}>();

const emit = defineEmits<{
  close: [];
  setSpeed: [s: number];
  setAudio: [id: number];
  setSub: [id: number];
  loadSubtitle: [];
  screenshot: [withSubs: boolean];
  setAbA: [];
  setAbB: [];
  clearAb: [];
  frameBack: [];
  frameForward: [];
  rotateCw: [];
  rotateCcw: [];
  toggleHFlip: [];
  toggleVFlip: [];
  resetTransform: [];
}>();

function trackLabel(t: Track, idx: number): string {
  const parts = [];
  if (t.lang) parts.push(t.lang);
  if (t.title) parts.push(t.title);
  if (t.external) parts.push("(外挂)");
  if (parts.length === 0) parts.push(`轨道 ${idx + 1}`);
  return parts.join(" · ");
}
</script>

<template>
  <div class="panel" @click.stop @dblclick.stop @wheel.stop>
    <!-- 头部 -->
    <div class="panel-head">
      <svg viewBox="0 0 24 24" class="head-icon"><circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="2.4" fill="currentColor" stroke="none"/></svg>
      <span class="head-title">播放操作</span>
      <button class="close-btn" @click="emit('close')">
        <svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6 6 18"/></svg>
      </button>
    </div>

    <div class="panel-body">
      <!-- 倍速 -->
      <section class="card">
        <div class="card-label">播放速度</div>
        <div class="speed-grid">
          <button
            v-for="s in SPEED_PRESETS"
            :key="s"
            class="chip"
            :class="{ active: Math.abs(speed - s) < 0.01 }"
            @click="emit('setSpeed', s)"
          >
            {{ s }}<span class="x">×</span>
          </button>
        </div>
      </section>

      <!-- 音轨 -->
      <section class="card">
        <div class="card-label">音轨</div>
        <div class="track-list">
          <button
            v-for="(t, i) in audioTracks"
            :key="'a' + t.id"
            class="track-item"
            :class="{ active: t.id === currentAudioId }"
            @click="emit('setAudio', t.id)"
          >
            <span class="track-idx">#{{ t.id }}</span>
            <span class="track-name">{{ trackLabel(t, i) }}</span>
          </button>
          <p v-if="audioTracks.length === 0" class="empty">仅一个音轨</p>
        </div>
      </section>

      <!-- 字幕 -->
      <section class="card">
        <div class="card-label-row">
          <span class="card-label">字幕</span>
          <button class="link-btn" @click="emit('loadSubtitle')">
            <svg viewBox="0 0 24 24" class="ico-sm"><path d="M12 5v14M5 12h14"/></svg>外挂
          </button>
        </div>
        <div class="track-list">
          <button
            class="track-item"
            :class="{ active: currentSubId === 0 }"
            @click="emit('setSub', 0)"
          >
            <span class="track-name muted">禁用</span>
          </button>
          <button
            v-for="(t, i) in subTracks"
            :key="'s' + t.id"
            class="track-item"
            :class="{ active: t.id === currentSubId }"
            @click="emit('setSub', t.id)"
          >
            <span class="track-idx">#{{ t.id }}</span>
            <span class="track-name">{{ trackLabel(t, i) }}</span>
          </button>
        </div>
      </section>

      <!-- A-B 循环 -->
      <section class="card">
        <div class="card-label">A-B 循环</div>
        <div class="btn-row">
          <button class="pill" @click="emit('setAbA')">
            A 点<span v-if="abLoopA !== null" class="val">{{ abLoopA.toFixed(1) }}s</span>
          </button>
          <button class="pill" @click="emit('setAbB')">
            B 点<span v-if="abLoopB !== null" class="val">{{ abLoopB.toFixed(1) }}s</span>
          </button>
          <button class="pill warn" @click="emit('clearAb')">清除</button>
        </div>
      </section>

      <!-- 逐帧 + 截图（合并一行，更紧凑） -->
      <section class="card">
        <div class="card-label">逐帧 / 截图</div>
        <div class="btn-row">
          <button class="pill" @click="emit('frameBack')">◀ 帧</button>
          <button class="pill" @click="emit('frameForward')">帧 ▶</button>
          <button class="pill" @click="emit('screenshot', true)">字幕截图</button>
          <button class="pill" @click="emit('screenshot', false)">画面截图</button>
        </div>
      </section>

      <!-- 画面变换 -->
      <section class="card">
        <div class="card-label-row">
          <span class="card-label">画面变换</span>
          <button class="link-btn" @click="emit('resetTransform')" title="还原全部">还原</button>
        </div>
        <div class="btn-row">
          <button class="pill" :class="{ on: videoRotate % 360 !== 0 }" @click="emit('rotateCw')">
            旋转<span class="val">{{ videoRotate }}°</span>
          </button>
          <button class="pill" @click="emit('rotateCcw')">↺ 90°</button>
          <button class="pill" :class="{ on: hFlipped }" @click="emit('toggleHFlip')">水平翻转</button>
          <button class="pill" :class="{ on: vFlipped }" @click="emit('toggleVFlip')">垂直翻转</button>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.panel {
  width: 300px;
  max-height: 70vh;
  display: flex;
  flex-direction: column;
  border-radius: 16px;
  background: rgba(20, 20, 26, 0.88);
  backdrop-filter: blur(24px) saturate(150%);
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.55);
  overflow: hidden;
}

/* —— 头部 —— */
.panel-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 13px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}
.head-icon {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: var(--color-accent);
  stroke-width: 1.8;
}
.head-title {
  font-size: 13px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.95);
  flex: 1;
}
.close-btn {
  width: 24px;
  height: 24px;
  border-radius: 7px;
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s ease;
}
.close-btn svg {
  width: 14px;
  height: 14px;
  fill: none;
  stroke: rgba(255, 255, 255, 0.55);
  stroke-width: 2;
  stroke-linecap: round;
}
.close-btn:hover {
  background: rgba(255, 255, 255, 0.1);
}
.close-btn:hover svg {
  stroke: #fff;
}

/* —— body / card 分组卡片 —— */
.panel-body {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.card {
  background: rgba(255, 255, 255, 0.035);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 11px;
  padding: 11px 12px;
}
.card-label {
  font-size: 10.5px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: rgba(255, 255, 255, 0.42);
  margin-bottom: 9px;
}
.card-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 9px;
}
.card-label-row .card-label {
  margin-bottom: 0;
}

.link-btn {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 11px;
  color: var(--color-accent);
  background: transparent;
  padding: 2px 6px;
  border-radius: 6px;
}
.link-btn:hover {
  background: rgba(78, 161, 255, 0.12);
}
.ico-sm {
  width: 11px;
  height: 11px;
  fill: none;
  stroke: currentColor;
  stroke-width: 2.4;
  stroke-linecap: round;
}

/* —— 倍速 chip —— */
.speed-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 5px;
}
.chip {
  padding: 6px 0;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.75);
  background: rgba(255, 255, 255, 0.06);
  transition: all 0.15s ease;
}
.chip .x {
  opacity: 0.5;
  font-size: 10px;
}
.chip:hover {
  background: rgba(255, 255, 255, 0.14);
  color: #fff;
}
.chip.active {
  background: var(--color-accent);
  color: #fff;
  box-shadow: 0 2px 8px rgba(78, 161, 255, 0.4);
}
.chip.active .x {
  opacity: 0.8;
}

/* —— 轨道列表 —— */
.track-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.track-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  font-size: 12px;
  text-align: left;
  color: rgba(255, 255, 255, 0.78);
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid transparent;
  transition: all 0.15s ease;
}
.track-item:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}
.track-item.active {
  background: rgba(78, 161, 255, 0.18);
  border-color: rgba(78, 161, 255, 0.4);
  color: #fff;
}
.track-idx {
  font-size: 10px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.4);
  background: rgba(255, 255, 255, 0.08);
  padding: 1px 5px;
  border-radius: 4px;
  flex-shrink: 0;
}
.track-item.active .track-idx {
  color: #fff;
  background: rgba(78, 161, 255, 0.35);
}
.track-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.track-name.muted {
  color: rgba(255, 255, 255, 0.45);
}
.empty {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.4);
  padding: 6px 4px;
  text-align: center;
}

/* —— 通用 pill 按钮 —— */
.btn-row {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}
.pill {
  flex: 1;
  min-width: 62px;
  padding: 8px 6px;
  border-radius: 8px;
  font-size: 11.5px;
  color: rgba(255, 255, 255, 0.82);
  background: rgba(255, 255, 255, 0.06);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  transition: all 0.15s ease;
}
.pill:hover {
  background: rgba(255, 255, 255, 0.14);
  color: #fff;
}
.pill .val {
  font-size: 10px;
  color: var(--color-accent);
  font-weight: 600;
}
.pill.on {
  background: rgba(78, 161, 255, 0.22);
  color: #fff;
  box-shadow: inset 0 0 0 1px rgba(78, 161, 255, 0.45);
}
.pill.warn:hover {
  background: rgba(255, 90, 90, 0.22);
  color: #ff8a8a;
}
</style>
