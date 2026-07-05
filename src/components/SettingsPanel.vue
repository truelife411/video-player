<script setup lang="ts">
import { ref } from "vue";
import type { TagType } from "../composables/useTags";

defineProps<{
  skipSeconds: number;
  windowScale: number;
  resumeMode: "start" | "resume";
  tagTypes: TagType[];
}>();

const emit = defineEmits<{
  close: [];
  setSkipSeconds: [s: number];
  setWindowScale: [s: number];
  setResumeMode: [m: "start" | "resume"];
  createTagType: [name: string, valueType: "enum" | "free", options: string[]];
  deleteTagType: [typeId: number];
}>();

// 标签管理：新建表单
const showCreateTag = ref(false);
const newTagName = ref("");
const newTagType = ref<"enum" | "free">("enum");
const newTagOptions = ref("");

function submitCreateTag() {
  const name = newTagName.value.trim();
  if (!name) return;
  const opts =
    newTagType.value === "enum"
      ? newTagOptions.value
          .split(/[,\n]/)
          .map((s) => s.trim())
          .filter(Boolean)
      : [];
  emit("createTagType", name, newTagType.value, opts);
  showCreateTag.value = false;
  newTagName.value = "";
  newTagOptions.value = "";
}

// 删除确认：用自定义模态替代 window.confirm（Tauri WebView2 不支持原生 dialog）
const pendingDelete = ref<TagType | null>(null);

function confirmDeleteTag(t: TagType) {
  if (t.is_preset) return;
  pendingDelete.value = t;
}

function doDeleteTag() {
  if (pendingDelete.value) {
    emit("deleteTagType", pendingDelete.value.id);
    pendingDelete.value = null;
  }
}

function cancelDeleteTag() {
  pendingDelete.value = null;
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

      <!-- 窗口缩放系数 -->
      <section class="sec">
        <div class="sec-title">窗口缩放系数：{{ windowScale.toFixed(2) }}x</div>
        <div class="ab-row">
          <input
            type="range"
            min="0.25"
            max="3"
            step="0.05"
            :value="windowScale"
            @input="(e) => emit('setWindowScale', Number((e.target as HTMLInputElement).value))"
            style="flex:1; cursor:pointer"
          />
        </div>
        <p class="empty" style="padding-top:6px">控制打开视频时窗口相对原寸的放大倍数，超出屏幕仍会自动最大化</p>
      </section>

      <!-- 播放起点 -->
      <section class="sec">
        <div class="sec-title">新视频播放起点</div>
        <div class="type-toggle">
          <button
            :class="{ active: resumeMode === 'start' }"
            @click="emit('setResumeMode', 'start')"
          >
            从头开始
          </button>
          <button
            :class="{ active: resumeMode === 'resume' }"
            @click="emit('setResumeMode', 'resume')"
          >
            从上次位置
          </button>
        </div>
      </section>

      <!-- 标签管理 -->
      <section class="sec">
        <div class="sec-title-row">
          <span class="sec-title">标签管理</span>
          <button class="link-btn" @click="showCreateTag = !showCreateTag">
            + 新建标签
          </button>
        </div>

        <!-- 新建表单 -->
        <Transition name="expand">
          <div v-if="showCreateTag" class="create-tag-form">
            <input class="text-input" v-model="newTagName" placeholder="标签名（如：导演）" />
            <div class="type-toggle">
              <button :class="{ active: newTagType === 'enum' }" @click="newTagType = 'enum'">
                枚举（下拉）
              </button>
              <button :class="{ active: newTagType === 'free' }" @click="newTagType = 'free'">
                自由（文本）
              </button>
            </div>
            <textarea
              v-if="newTagType === 'enum'"
              class="textarea"
              v-model="newTagOptions"
              placeholder="候选值，逗号或换行分隔"
              rows="2"
            ></textarea>
            <button class="submit-btn" @click="submitCreateTag" :disabled="!newTagName.trim()">
              创建
            </button>
          </div>
        </Transition>

        <!-- 已有标签类型列表 -->
        <div class="tag-type-list">
          <div v-for="t in tagTypes" :key="t.id" class="tag-type-item">
            <div class="tag-type-info">
              <span class="tag-type-name">{{ t.name }}</span>
              <span class="tag-kind" v-if="t.is_preset">系统</span>
              <span class="tag-kind custom" v-else>自定义</span>
              <span class="tag-vtype">{{ t.value_type === "enum" ? "枚举" : "自由" }}</span>
            </div>
            <button
              v-if="!t.is_preset"
              class="tag-del-btn"
              title="删除此标签类型"
              @click="confirmDeleteTag(t)"
            >
              ✕
            </button>
          </div>
          <p v-if="tagTypes.length === 0" class="empty">暂无标签类型</p>
        </div>
      </section>
    </div>

    <!-- 删除确认模态（替代 window.confirm，Tauri WebView2 不支持原生 dialog） -->
    <Transition name="pop">
      <div v-if="pendingDelete" class="confirm-mask" @click.self="cancelDeleteTag">
        <div class="confirm-box">
          <div class="confirm-title">删除标签类型</div>
          <p class="confirm-msg">
            确定要删除标签类型"{{ pendingDelete.name }}"吗？<br />
            删除后所有视频中该标签的值也会被清除。
          </p>
          <div class="confirm-actions">
            <button class="confirm-btn cancel" @click="cancelDeleteTag">取消</button>
            <button class="confirm-btn danger" @click="doDeleteTag">删除</button>
          </div>
        </div>
      </div>
    </Transition>
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

/* 标签管理 */
.create-tag-form {
  margin-bottom: 10px;
  padding: 10px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.text-input,
.textarea {
  width: 100%;
  padding: 7px 10px;
  border-radius: 7px;
  font-size: 12px;
  font-family: inherit;
  color: #fff;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.1);
  outline: none;
}
.text-input:focus,
.textarea:focus {
  border-color: rgba(78, 161, 255, 0.6);
}
.textarea {
  resize: vertical;
  font-family: inherit;
}

.type-toggle {
  display: flex;
  gap: 4px;
}
.type-toggle button {
  flex: 1;
  padding: 5px;
  border-radius: 5px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.7);
  background: rgba(255, 255, 255, 0.05);
}
.type-toggle button.active {
  background: rgba(78, 161, 255, 0.25);
  color: #fff;
}

.submit-btn {
  padding: 7px;
  border-radius: 7px;
  font-size: 12px;
  color: #fff;
  background: #4ea1ff;
}
.submit-btn:disabled {
  opacity: 0.4;
}

.tag-type-list {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.tag-type-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 7px 10px;
  border-radius: 7px;
  background: rgba(255, 255, 255, 0.04);
}

.tag-type-info {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.tag-type-name {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.9);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tag-kind {
  font-size: 10px;
  padding: 1px 5px;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.12);
  color: rgba(255, 255, 255, 0.6);
  flex-shrink: 0;
}
.tag-kind.custom {
  background: rgba(78, 161, 255, 0.2);
  color: #6db8ff;
}

.tag-vtype {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.4);
  flex-shrink: 0;
}

.tag-del-btn {
  width: 22px;
  height: 22px;
  border-radius: 5px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  background: transparent;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
.tag-del-btn:hover {
  background: rgba(255, 80, 80, 0.25);
  color: #ff6b6b;
}

/* 展开/收起动画 */
.expand-enter-active,
.expand-leave-active {
  transition: max-height 0.25s ease, opacity 0.25s ease, margin 0.25s ease;
  overflow: hidden;
}
.expand-enter-from,
.expand-leave-to {
  max-height: 0;
  opacity: 0;
  margin-bottom: 0;
}
.expand-enter-to,
.expand-leave-from {
  max-height: 200px;
}

/* 删除确认模态 */
.confirm-mask {
  position: fixed;
  inset: 0;
  z-index: 10000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(4px);
}

.confirm-box {
  width: 280px;
  padding: 18px;
  border-radius: 12px;
  background: rgba(28, 28, 34, 0.96);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.confirm-title {
  font-size: 15px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.95);
}

.confirm-msg {
  font-size: 13px;
  line-height: 1.5;
  color: rgba(255, 255, 255, 0.75);
}

.confirm-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.confirm-btn {
  padding: 7px 16px;
  border-radius: 8px;
  font-size: 13px;
  transition: background 0.15s ease;
}

.confirm-btn.cancel {
  color: rgba(255, 255, 255, 0.7);
  background: rgba(255, 255, 255, 0.08);
}
.confirm-btn.cancel:hover {
  background: rgba(255, 255, 255, 0.16);
}

.confirm-btn.danger {
  color: #fff;
  background: rgba(232, 70, 70, 0.9);
}
.confirm-btn.danger:hover {
  background: rgba(232, 70, 70, 1);
}
</style>
