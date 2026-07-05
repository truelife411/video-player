<script setup lang="ts">
import { ref, nextTick, computed } from "vue";
import type { VideoInfo } from "../composables/useTags";
import { formatSize, formatDate } from "../utils/format";

const props = defineProps<{
  keyword: string;
  results: VideoInfo[];
  searching: boolean;
}>();

const emit = defineEmits<{
  close: [];
  updatekeyword: [k: string];
  play: [path: string];
  reveal: [path: string];
}>();

const inputRef = ref<HTMLInputElement | null>(null);

// 打开时自动聚焦输入框
async function focusInput() {
  await nextTick();
  inputRef.value?.focus();
  inputRef.value?.select();
}
focusInput();

// 右键菜单（在文件夹中显示）
const menuIndex = ref<number | null>(null);
function onRowContextmenu(e: MouseEvent, idx: number) {
  e.preventDefault();
  menuIndex.value = menuIndex.value === idx ? null : idx;
}
function closeMenu() {
  menuIndex.value = null;
}

function onInput(e: Event) {
  emit("updatekeyword", (e.target as HTMLInputElement).value);
}

// 点击遮罩关闭
function onBackdropClick() {
  emit("close");
}

function onRowClick(v: VideoInfo) {
  emit("play", v.file_path);
  emit("close");
}

const hasKeyword = computed(() => props.keyword.trim().length > 0);
</script>

<template>
  <div class="backdrop" @click="onBackdropClick">
    <div class="overlay" @click.stop>
      <!-- 搜索框 -->
      <div class="search-bar">
        <span class="icon">🔍</span>
        <input
          ref="inputRef"
          class="input"
          type="text"
          :value="keyword"
          @input="onInput"
          @keydown.esc="emit('close')"
          placeholder="搜索视频名或标签（如：张艺谋、5、1080p）"
        />
        <span class="hint">Esc 关闭</span>
      </div>

      <!-- 结果列表 -->
      <div class="results" @click="closeMenu">
        <div v-if="hasKeyword && !searching && results.length === 0" class="empty">
          未找到匹配的视频
        </div>
        <div v-if="!hasKeyword" class="empty">输入关键词搜索（按文件名或标签）</div>

        <div
          v-for="(v, i) in results"
          :key="v.hash"
          class="row"
          @click="onRowClick(v)"
          @contextmenu="onRowContextmenu($event, i)"
        >
          <div class="row-main">
            <div class="row-name">{{ v.file_name }}</div>
            <div class="row-path">{{ v.file_path }}</div>
          </div>
          <div class="row-meta">
            <span class="tag-pill">{{ v.extension.toUpperCase() }}</span>
            <span>{{ formatSize(v.size_bytes) }}</span>
            <span>{{ formatDate(v.modified_at) }}</span>
          </div>

          <!-- 右键菜单 -->
          <div v-if="menuIndex === i" class="ctx-menu" @click.stop>
            <button @click="emit('reveal', v.file_path); menuIndex = null">
              📂 在文件夹中显示
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(6px);
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 12vh;
  z-index: 10000;
}

.overlay {
  width: 640px;
  max-width: 90vw;
  max-height: 70vh;
  display: flex;
  flex-direction: column;
  border-radius: 16px;
  background: rgba(22, 22, 28, 0.92);
  backdrop-filter: blur(28px) saturate(160%);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
  overflow: hidden;
}

.search-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 18px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.icon {
  font-size: 18px;
  opacity: 0.7;
}

.input {
  flex: 1;
  font-size: 15px;
  font-family: inherit;
  color: #fff;
  background: transparent;
  border: none;
  outline: none;
}
.input::placeholder {
  color: rgba(255, 255, 255, 0.35);
}

.hint {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
}

.results {
  flex: 1;
  overflow-y: auto;
  padding: 6px;
}

.empty {
  padding: 32px;
  text-align: center;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.4);
}

.row {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  cursor: pointer;
  transition: background 0.12s ease;
}
.row:hover {
  background: rgba(255, 255, 255, 0.08);
}

.row-main {
  flex: 1;
  min-width: 0;
}

.row-name {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.95);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row-path {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row-meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.5);
  flex-shrink: 0;
}

.tag-pill {
  padding: 1px 7px;
  border-radius: 4px;
  background: rgba(78, 161, 255, 0.25);
  color: #6db8ff;
  font-size: 10px;
  font-weight: 600;
}

.ctx-menu {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  background: rgba(40, 40, 48, 0.98);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  padding: 4px;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5);
  z-index: 5;
}
.ctx-menu button {
  display: block;
  width: 100%;
  padding: 7px 12px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.9);
  background: transparent;
  border-radius: 5px;
  text-align: left;
  white-space: nowrap;
}
.ctx-menu button:hover {
  background: rgba(255, 255, 255, 0.1);
}
</style>
