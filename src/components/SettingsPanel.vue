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
  skipSeconds: number;
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
  setSkipSeconds: [s: number];
}>();

function trackLabel(t: Track, idx: number): string {
  const parts = [`#${t.id}`];
  if (t.lang) parts.push(t.lang);
  if (t.title) parts.push(t.title);
  if (t.external) parts.push("(外挂)");
  if (parts.length === 1) parts.push(`轨道 ${idx + 1}`);
  return parts.join(" · ");
}
</script>

<template>
  <div class="panel" @click.stop @dblclick.stop @wheel.stop>
    <!-- 头部 -->
    <div class="panel-head">
      <span>设置</span>
      <button class="close-btn" @click="emit('close')">✕</button>
    </div>

    <div class="panel-body">
      <!-- 倍速 -->
      <section class="sec">
        <div class="sec-title">播放速度</div>
        <div class="speed-row">
          <button
            v-for="s in SPEED_PRESETS"
            :key="s"
            class="chip"
            :class="{ active: Math.abs(speed - s) < 0.01 }"
            @click="emit('setSpeed', s)"
          >
            {{ s }}x
          </button>
        </div>
      </section>

      <!-- 音轨 -->
      <section class="sec">
        <div class="sec-title">音轨</div>
        <div class="track-list">
          <button
            v-for="(t, i) in audioTracks"
            :key="'a' + t.id"
            class="track-item"
            :class="{ active: t.id === currentAudioId }"
            @click="emit('setAudio', t.id)"
          >
            {{ trackLabel(t, i) }}
          </button>
          <p v-if="audioTracks.length === 0" class="empty">仅一个音轨</p>
        </div>
      </section>

      <!-- 字幕 -->
      <section class="sec">
        <div class="sec-title-row">
          <span class="sec-title">字幕</span>
          <button class="link-btn" @click="emit('loadSubtitle')">+ 外挂字幕</button>
        </div>
        <div class="track-list">
          <button
            class="track-item"
            :class="{ active: currentSubId === 0 }"
            @click="emit('setSub', 0)"
          >
            禁用
          </button>
          <button
            v-for="(t, i) in subTracks"
            :key="'s' + t.id"
            class="track-item"
            :class="{ active: t.id === currentSubId }"
            @click="emit('setSub', t.id)"
          >
            {{ trackLabel(t, i) }}
          </button>
        </div>
      </section>

      <!-- A-B 循环 -->
      <section class="sec">
        <div class="sec-title">A-B 循环</div>
        <div class="ab-row">
          <button class="ab-btn" @click="emit('setAbA')">
            设 A 点<span v-if="abLoopA !== null">{{ abLoopA.toFixed(1) }}s</span>
          </button>
          <button class="ab-btn" @click="emit('setAbB')">
            设 B 点<span v-if="abLoopB !== null">{{ abLoopB.toFixed(1) }}s</span>
          </button>
          <button class="ab-btn warn" @click="emit('clearAb')">清除</button>
        </div>
      </section>

      <!-- 逐帧 -->
      <section class="sec">
        <div class="sec-title">逐帧</div>
        <div class="ab-row">
          <button class="ab-btn" @click="emit('frameBack')">◀ 上一帧</button>
          <button class="ab-btn" @click="emit('frameForward')">下一帧 ▶</button>
        </div>
      </section>

      <!-- 快进快退时间 -->
      <section class="sec">
        <div class="sec-title">快进/快退时间：{{ skipSeconds }} 秒</div>
        <div class="ab-row">
          <input
            type="range"
            min="1"
            max="60"
            step="1"
            :value="skipSeconds"
            @input="(e) => emit('setSkipSeconds', Number((e.target as HTMLInputElement).value))"
            style="flex:1; cursor:pointer"
          />
        </div>
      </section>

      <!-- 截图 -->
      <section class="sec">
        <div class="sec-title">截图</div>
        <div class="ab-row">
          <button class="ab-btn" @click="emit('screenshot', true)">含字幕</button>
          <button class="ab-btn" @click="emit('screenshot', false)">仅画面</button>
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
  border-radius: 14px;
  background: rgba(22, 22, 28, 0.85);
  backdrop-filter: blur(20px) saturate(140%);
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
  overflow: hidden;
}

.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  font-size: 14px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.95);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.close-btn {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.6);
  background: transparent;
  font-size: 13px;
}
.close-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.panel-body {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

.sec {
  padding: 12px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}
.sec:last-child {
  border-bottom: none;
}

.sec-title {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: rgba(255, 255, 255, 0.45);
  margin-bottom: 8px;
}
.sec-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.link-btn {
  font-size: 12px;
  color: var(--color-accent, #4ea1ff);
  background: transparent;
}
.link-btn:hover {
  text-decoration: underline;
}

/* 倍速 chips */
.speed-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.chip {
  padding: 5px 10px;
  border-radius: 7px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.8);
  background: rgba(255, 255, 255, 0.08);
  transition: background 0.15s ease;
}
.chip:hover {
  background: rgba(255, 255, 255, 0.16);
}
.chip.active {
  background: #4ea1ff;
  color: #fff;
}

/* 轨道列表 */
.track-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.track-item {
  padding: 7px 10px;
  border-radius: 7px;
  font-size: 12px;
  text-align: left;
  color: rgba(255, 255, 255, 0.8);
  background: rgba(255, 255, 255, 0.05);
  transition: background 0.15s ease;
}
.track-item:hover {
  background: rgba(255, 255, 255, 0.12);
}
.track-item.active {
  background: rgba(78, 161, 255, 0.25);
  color: #fff;
}
.empty {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.4);
  padding: 6px 0;
}

/* AB / 逐帧 / 截图 行 */
.ab-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.ab-btn {
  flex: 1;
  min-width: 80px;
  padding: 7px 8px;
  border-radius: 7px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.85);
  background: rgba(255, 255, 255, 0.08);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  transition: background 0.15s ease;
}
.ab-btn:hover {
  background: rgba(255, 255, 255, 0.16);
}
.ab-btn span {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.5);
}
.ab-btn.warn:hover {
  background: rgba(255, 90, 90, 0.25);
}
</style>
