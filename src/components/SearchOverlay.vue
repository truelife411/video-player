<script setup lang="ts">
import { ref, nextTick, computed, onMounted, onUnmounted, watch } from "vue";
import type { VideoInfo } from "../composables/useTags";
import { formatSize, formatDate } from "../utils/format";

const props = defineProps<{
  keyword: string;
  selectedStars: number | null;
  results: VideoInfo[];
  searching: boolean;
}>();

const emit = defineEmits<{
  close: [];
  updatekeyword: [k: string];
  selectStars: [n: number];
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

// 全局监听 ESC 关闭（不依赖 input 焦点）
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}
onMounted(() => window.addEventListener("keydown", onKeydown));
onUnmounted(() => window.removeEventListener("keydown", onKeydown));

function onInput(e: Event) {
  emit("updatekeyword", (e.target as HTMLInputElement).value);
}

// —— 排序 ——
type SortKey = "file_name" | "file_path" | "size_bytes" | "modified_at" | "stars" | "quality";
type SortDir = "asc" | "desc";
const sortKey = ref<SortKey>("modified_at");
const sortDir = ref<SortDir>("desc");

// 画质排序权重映射（按清晰度从低到高）
const QUALITY_RANK: Record<string, number> = {
  "480p": 1, "720p": 2, "1080p": 3, "4K": 4,
};

const sortedResults = computed(() => {
  const arr = [...props.results];
  const key = sortKey.value;
  const dir = sortDir.value === "asc" ? 1 : -1;
  arr.sort((a, b) => {
    let va: string | number = a[key];
    let vb: string | number = b[key];
    // 画质用权重排序
    if (key === "quality") {
      va = QUALITY_RANK[a.quality] || 0;
      vb = QUALITY_RANK[b.quality] || 0;
    }
    if (typeof va === "string" && typeof vb === "string") {
      return va.localeCompare(vb, "zh") * dir;
    }
    return ((va as number) - (vb as number)) * dir;
  });
  return arr;
});

function toggleSort(key: SortKey) {
  if (sortKey.value === key) {
    sortDir.value = sortDir.value === "asc" ? "desc" : "asc";
  } else {
    sortKey.value = key;
    sortDir.value = "desc"; // 新列默认降序（修改时间等数值列直觉）
  }
}

// —— 分页 ——
const PAGE_SIZE = 50;
const currentPage = ref(1);

// 关键词或星级变化时重置到第一页
watch(
  () => [props.keyword, props.selectedStars],
  () => { currentPage.value = 1; }
);

const totalPages = computed(() => Math.max(1, Math.ceil(sortedResults.value.length / PAGE_SIZE)));
const pagedResults = computed(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE;
  return sortedResults.value.slice(start, start + PAGE_SIZE);
});

function goToPage(p: number) {
  currentPage.value = Math.max(1, Math.min(totalPages.value, p));
}

// 表头配置
const columns: { key: SortKey; label: string; cls: string }[] = [
  { key: "file_name", label: "文件名", cls: "col-name" },
  { key: "file_path", label: "文件路径", cls: "col-path" },
  { key: "size_bytes", label: "大小", cls: "col-size" },
  { key: "modified_at", label: "修改时间", cls: "col-time" },
  { key: "stars", label: "星级", cls: "col-stars" },
  { key: "quality", label: "画质", cls: "col-quality" },
];

function starsText(s: number): string {
  return s > 0 ? "★".repeat(Math.min(7, s)) : "—";
}

function onRowClick(v: VideoInfo) {
  emit("play", v.file_path);
  emit("close");
}

// 有筛选条件：关键词或星级任一存在（决定空状态文案/结果显示）
const hasFilter = computed(
  () => props.keyword.trim().length > 0 || props.selectedStars !== null
);

// 翻页栏组件（上下复用）
function pageNumbers(): number[] {
  const total = totalPages.value;
  const cur = currentPage.value;
  const out: number[] = [];
  // 显示首尾 + 当前页前后 2 页
  const add = (n: number) => { if (!out.includes(n) && n >= 1 && n <= total) out.push(n); };
  add(1);
  for (let i = cur - 2; i <= cur + 2; i++) add(i);
  add(total);
  return out.sort((a, b) => a - b);
}
</script>

<template>
  <div class="backdrop" @click="emit('close')">
    <div class="overlay" @click.stop @wheel.stop>
      <!-- 头部：标题 + 关闭按钮 -->
      <div class="head">
        <span class="title">🔍 搜索视频</span>
        <button class="close-btn" @click="emit('close')" title="关闭 (Esc)">✕</button>
      </div>

      <!-- 搜索框 + 星级快捷按钮 -->
      <div class="search-bar">
        <input
          ref="inputRef"
          class="input"
          type="text"
          :value="keyword"
          @input="onInput"
          placeholder="搜索文件名或标签（如：张艺谋、1080p）"
        />
        <!-- 星级快捷筛选：1-7 数字按钮，点击纯按星级筛选（再点取消） -->
        <div class="star-quick">
          <span class="star-label">星级</span>
          <button
            v-for="n in 7"
            :key="n"
            class="star-btn"
            :class="{ active: selectedStars === n }"
            :title="`${n} 星视频`"
            @click="emit('selectStars', n)"
          >{{ n }}</button>
        </div>
        <span class="count" v-if="hasFilter && !searching">{{ results.length }} 个结果</span>
        <span class="count searching" v-if="searching">搜索中…</span>
      </div>

      <!-- 上翻页栏 -->
      <div class="pager" v-if="totalPages > 1">
        <button class="pg-btn" :disabled="currentPage === 1" @click="goToPage(1)">«</button>
        <button class="pg-btn" :disabled="currentPage === 1" @click="goToPage(currentPage - 1)">‹</button>
        <button
          v-for="p in pageNumbers()"
          :key="p"
          class="pg-num"
          :class="{ active: p === currentPage }"
          @click="goToPage(p)"
        >{{ p }}</button>
        <button class="pg-btn" :disabled="currentPage === totalPages" @click="goToPage(currentPage + 1)">›</button>
        <button class="pg-btn" :disabled="currentPage === totalPages" @click="goToPage(totalPages)">»</button>
      </div>

      <!-- 表格 -->
      <div class="table-wrap">
        <table class="table">
          <thead>
            <tr>
              <th
                v-for="col in columns"
                :key="col.key"
                :class="[col.cls, { sortable: true, active: sortKey === col.key }]"
                @click="toggleSort(col.key)"
              >
                {{ col.label }}
                <span class="sort-arrow" v-if="sortKey === col.key">{{ sortDir === "asc" ? "▲" : "▼" }}</span>
              </th>
              <th class="col-action">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="!hasFilter">
              <td colspan="7" class="empty-row">输入关键词或点击星级按钮搜索</td>
            </tr>
            <tr v-else-if="searching">
              <td colspan="7" class="empty-row">搜索中…</td>
            </tr>
            <tr v-else-if="results.length === 0">
              <td colspan="7" class="empty-row">未找到匹配的视频</td>
            </tr>
            <tr v-for="v in pagedResults" :key="v.hash" class="data-row" @click="onRowClick(v)">
              <td class="col-name" :title="v.file_name">{{ v.file_name }}</td>
              <td class="col-path" :title="v.file_path">{{ v.file_path }}</td>
              <td class="col-size">{{ formatSize(v.size_bytes) }}</td>
              <td class="col-time">{{ formatDate(v.modified_at) }}</td>
              <td class="col-stars">{{ starsText(v.stars) }}</td>
              <td class="col-quality">{{ v.quality || "—" }}</td>
              <td class="col-action" @click.stop>
                <button class="reveal-btn" title="在文件夹中显示" @click="emit('reveal', v.file_path)">📂</button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- 下翻页栏 -->
      <div class="pager pager-bottom" v-if="totalPages > 1">
        <span class="page-info">第 {{ currentPage }} / {{ totalPages }} 页（共 {{ results.length }} 条）</span>
        <div class="pager-btns">
          <button class="pg-btn" :disabled="currentPage === 1" @click="goToPage(1)">«</button>
          <button class="pg-btn" :disabled="currentPage === 1" @click="goToPage(currentPage - 1)">‹</button>
          <button
            v-for="p in pageNumbers()"
            :key="p"
            class="pg-num"
            :class="{ active: p === currentPage }"
            @click="goToPage(p)"
          >{{ p }}</button>
          <button class="pg-btn" :disabled="currentPage === totalPages" @click="goToPage(currentPage + 1)">›</button>
          <button class="pg-btn" :disabled="currentPage === totalPages" @click="goToPage(totalPages)">»</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10000;
}

.overlay {
  width: 88vw;
  max-width: 1200px;
  height: 78vh;
  display: flex;
  flex-direction: column;
  border-radius: 16px;
  background: rgba(20, 20, 26, 0.94);
  backdrop-filter: blur(28px) saturate(150%);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
  overflow: hidden;
}

/* 头部 */
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}
.title {
  font-size: 14px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.95);
}
.close-btn {
  width: 26px;
  height: 26px;
  border-radius: 7px;
  color: rgba(255, 255, 255, 0.55);
  background: transparent;
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s ease;
}
.close-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

/* 搜索框 */
.search-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 18px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}
.input {
  flex: 1;
  font-size: 14px;
  font-family: inherit;
  color: #fff;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  padding: 8px 12px;
  outline: none;
}
.input:focus {
  border-color: rgba(78, 161, 255, 0.6);
}
.input::placeholder {
  color: rgba(255, 255, 255, 0.35);
}
.count {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
  flex-shrink: 0;
}
.count.searching {
  color: var(--color-accent, #4ea1ff);
}

/* 星级快捷按钮 */
.star-quick {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}
.star-label {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  margin-right: 2px;
}
.star-btn {
  min-width: 24px;
  height: 24px;
  padding: 0 5px;
  border-radius: 6px;
  font-size: 11px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.6);
  background: rgba(255, 255, 255, 0.06);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.12s ease;
}
.star-btn:hover {
  background: rgba(255, 255, 255, 0.14);
  color: #fff;
}
.star-btn.active {
  background: rgba(255, 201, 77, 0.3);
  color: #ffd24d;
  box-shadow: inset 0 0 0 1px rgba(255, 201, 77, 0.5);
}

/* 翻页栏 */
.pager {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 8px 18px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}
.pager-bottom {
  border-bottom: none;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  justify-content: space-between;
}
.page-info {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.45);
}
.pager-btns {
  display: flex;
  align-items: center;
  gap: 4px;
}
.pg-btn, .pg-num {
  min-width: 28px;
  height: 28px;
  padding: 0 6px;
  border-radius: 6px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.7);
  background: rgba(255, 255, 255, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.12s ease;
}
.pg-btn:hover:not(:disabled), .pg-num:hover {
  background: rgba(255, 255, 255, 0.14);
  color: #fff;
}
.pg-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}
.pg-num.active {
  background: var(--color-accent, #4ea1ff);
  color: #fff;
}

/* 表格 */
.table-wrap {
  flex: 1;
  overflow: auto;
}
.table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12.5px;
}
.table thead {
  position: sticky;
  top: 0;
  z-index: 2;
}
.table th {
  padding: 9px 12px;
  text-align: left;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: rgba(255, 255, 255, 0.5);
  background: rgba(28, 28, 36, 0.98);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  white-space: nowrap;
  user-select: none;
}
.table th.sortable {
  cursor: pointer;
}
.table th.sortable:hover {
  color: rgba(255, 255, 255, 0.85);
}
.table th.active {
  color: var(--color-accent, #4ea1ff);
}
.sort-arrow {
  font-size: 9px;
  margin-left: 2px;
}
.table td {
  padding: 8px 12px;
  color: rgba(255, 255, 255, 0.82);
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}

/* 列宽控制 */
.col-name { min-width: 120px; max-width: 220px; }
.col-path { min-width: 150px; max-width: 300px; }
.col-size { width: 70px; white-space: nowrap; text-align: right; }
.col-time { width: 130px; white-space: nowrap; }
.col-stars { width: 90px; white-space: nowrap; color: #ffc94d; }
.col-quality { width: 70px; white-space: nowrap; }
.col-action { width: 50px; text-align: center; }

.col-name, .col-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 行交互 */
.data-row {
  cursor: pointer;
  transition: background 0.1s ease;
}
.data-row:hover {
  background: rgba(78, 161, 255, 0.1);
}
.data-row:nth-child(even) {
  background: rgba(255, 255, 255, 0.015);
}
.data-row:nth-child(even):hover {
  background: rgba(78, 161, 255, 0.1);
}

.empty-row {
  text-align: center;
  color: rgba(255, 255, 255, 0.4);
  padding: 40px;
  font-size: 13px;
}

.reveal-btn {
  font-size: 14px;
  background: transparent;
  opacity: 0.6;
  transition: opacity 0.12s ease;
}
.reveal-btn:hover {
  opacity: 1;
}
</style>
