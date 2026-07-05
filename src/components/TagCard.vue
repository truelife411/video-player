<script setup lang="ts">
import { ref, watch, computed } from "vue";
import type { TagType } from "../composables/useTags";

const props = defineProps<{
  tagTypes: TagType[];
  getValue: (typeId: number) => string;
  hash: string;
}>();

const emit = defineEmits<{
  close: [];
  setValue: [typeId: number, value: string];
  createType: [name: string, valueType: "enum" | "free", options: string[]];
}>();

// 星级专用渲染
function isStar(t: TagType): boolean {
  return t.is_preset && t.name === "星级";
}
function starCount(t: TagType): number {
  const v = props.getValue(t.id);
  return v ? parseInt(v, 10) || 0 : 0;
}
async function setStar(t: TagType, n: number) {
  // 再点同一颗星=清零
  const cur = starCount(t);
  emit("setValue", t.id, cur === n ? "" : String(n));
}

// 枚举型（非星级）：下拉
// 自由型：输入框

// 新建标签类型弹层
const showCreate = ref(false);
const newName = ref("");
const newType = ref<"enum" | "free">("enum");
const newOptions = ref("");

function submitCreate() {
  const name = newName.value.trim();
  if (!name) return;
  const opts =
    newType.value === "enum"
      ? newOptions.value
          .split(/[,\n]/)
          .map((s) => s.trim())
          .filter(Boolean)
      : [];
  emit("createType", name, newType.value, opts);
  showCreate.value = false;
  newName.value = "";
  newOptions.value = "";
}

// 缓存：自由型输入的本地值（避免每次输入都写库）
const freeInputs = ref<Record<number, string>>({});
watch(
  () => props.tagTypes,
  () => {
    // 初始化自由型输入框
    for (const t of props.tagTypes) {
      if (t.value_type === "free" && !(t.id in freeInputs.value)) {
        freeInputs.value[t.id] = props.getValue(t.id);
      }
    }
  },
  { immediate: true }
);

// 当外部标签更新时，同步自由型输入框
watch(
  () => props.hash,
  () => {
    freeInputs.value = {};
    for (const t of props.tagTypes) {
      if (t.value_type === "free") freeInputs.value[t.id] = props.getValue(t.id);
    }
  }
);

const title = computed(() => (props.hash ? "视频标签" : "无视频"));
</script>

<template>
  <div class="card" @click.stop @dblclick.stop @wheel.stop>
    <div class="head">
      <span class="title">{{ title }}</span>
      <button class="close-btn" @click="emit('close')">✕</button>
    </div>

    <div class="body">
      <!-- 标签列表 -->
      <div v-for="t in tagTypes" :key="t.id" class="row">
        <div class="row-label">
          {{ t.name }}
          <span class="tag-kind" v-if="!t.is_preset">自定义</span>
        </div>

        <!-- 星级（专用 5 颗星） -->
        <div v-if="isStar(t)" class="stars">
          <button
            v-for="n in 5"
            :key="n"
            class="star"
            :class="{ on: n <= starCount(t) }"
            @click="setStar(t, n)"
            :title="`${n} 星`"
          >
            ★
          </button>
        </div>

        <!-- 枚举型（下拉） -->
        <select
          v-else-if="t.value_type === 'enum'"
          class="select"
          :value="getValue(t.id)"
          @change="(e) => emit('setValue', t.id, (e.target as HTMLSelectElement).value)"
        >
          <option value="">未设置</option>
          <option v-for="opt in t.options" :key="opt" :value="opt">{{ opt }}</option>
        </select>

        <!-- 自由型（输入 + 保存按钮） -->
        <div v-else class="free-input">
          <input
            type="text"
            class="text-input"
            v-model="freeInputs[t.id]"
            placeholder="输入标签值"
          />
          <button class="save-btn" @click="emit('setValue', t.id, freeInputs[t.id] || '')">
            保存
          </button>
        </div>
      </div>

      <!-- 新建按钮 -->
      <button class="add-type-btn" @click="showCreate = !showCreate">+ 新建标签类型</button>

      <!-- 新建表单 -->
      <Transition name="expand">
        <div v-if="showCreate" class="create-form">
          <input class="text-input" v-model="newName" placeholder="标签名（如：导演）" />
          <div class="type-toggle">
            <button :class="{ active: newType === 'enum' }" @click="newType = 'enum'">
              枚举（下拉）
            </button>
            <button :class="{ active: newType === 'free' }" @click="newType = 'free'">
              自由（文本）
            </button>
          </div>
          <textarea
            v-if="newType === 'enum'"
            class="textarea"
            v-model="newOptions"
            placeholder="候选值，逗号或换行分隔，如：诺兰, 张艺谋, 卡梅隆"
            rows="3"
          ></textarea>
          <button class="submit-btn" @click="submitCreate" :disabled="!newName.trim()">
            创建
          </button>
        </div>
      </Transition>
    </div>
  </div>
</template>

<style scoped>
.card {
  width: 340px;
  max-height: 75vh;
  display: flex;
  flex-direction: column;
  border-radius: 16px;
  background: rgba(22, 22, 28, 0.85);
  backdrop-filter: blur(24px) saturate(160%);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
  overflow: hidden;
}

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
  color: rgba(255, 255, 255, 0.6);
  background: transparent;
  font-size: 13px;
}
.close-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.body {
  flex: 1;
  overflow-y: auto;
  padding: 8px 18px 16px;
}

.row {
  padding: 12px 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.row-label {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.6);
  margin-bottom: 8px;
  display: flex;
  align-items: center;
  gap: 6px;
}

.tag-kind {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(78, 161, 255, 0.2);
  color: #6db8ff;
}

/* 星级 */
.stars {
  display: flex;
  gap: 2px;
}
.star {
  font-size: 26px;
  line-height: 1;
  color: rgba(255, 255, 255, 0.25);
  background: transparent;
  padding: 0 2px;
  transition: color 0.15s ease, transform 0.1s ease;
}
.star:hover {
  transform: scale(1.15);
}
.star.on {
  color: #ffc83d;
  text-shadow: 0 0 8px rgba(255, 200, 61, 0.5);
}

/* 下拉/输入 */
.select,
.text-input,
.textarea {
  width: 100%;
  padding: 8px 10px;
  border-radius: 8px;
  font-size: 13px;
  font-family: inherit;
  color: #fff;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.1);
  outline: none;
}
.select option {
  background: #1a1a20;
}
.text-input:focus,
.textarea:focus {
  border-color: rgba(78, 161, 255, 0.6);
}
.textarea {
  resize: vertical;
  font-family: inherit;
}

.free-input {
  display: flex;
  gap: 6px;
}
.save-btn {
  padding: 0 14px;
  border-radius: 8px;
  font-size: 12px;
  color: #fff;
  background: rgba(78, 161, 255, 0.85);
  white-space: nowrap;
}
.save-btn:hover {
  background: #4ea1ff;
}

/* 新建 */
.add-type-btn {
  margin-top: 14px;
  width: 100%;
  padding: 10px;
  border-radius: 8px;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.7);
  background: rgba(255, 255, 255, 0.05);
  border: 1px dashed rgba(255, 255, 255, 0.2);
}
.add-type-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.create-form {
  margin-top: 10px;
  padding: 12px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.04);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.type-toggle {
  display: flex;
  gap: 4px;
}
.type-toggle button {
  flex: 1;
  padding: 6px;
  border-radius: 6px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.7);
  background: rgba(255, 255, 255, 0.05);
}
.type-toggle button.active {
  background: rgba(78, 161, 255, 0.25);
  color: #fff;
}

.submit-btn {
  padding: 8px;
  border-radius: 8px;
  font-size: 13px;
  color: #fff;
  background: #4ea1ff;
}
.submit-btn:disabled {
  opacity: 0.4;
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
  margin-top: 0;
}
.expand-enter-to,
.expand-leave-from {
  max-height: 300px;
}
</style>
