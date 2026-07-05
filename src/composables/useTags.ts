import { ref, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// 标签类型定义
export interface TagType {
  id: number;
  name: string;
  value_type: "enum" | "free";
  is_preset: boolean;
  sort_order: number;
  options: string[]; // 枚举候选值
}

export interface VideoTag {
  type_id: number;
  type_name: string;
  value_type: "enum" | "free";
  value: string;
}

export interface VideoInfo {
  hash: string;
  file_name: string;
  file_path: string;
  extension: string;
  size_bytes: number;
  modified_at: number;
  play_position: number;
  duration: number;
}

/**
 * 标签管理：CRUD 标签类型、读写视频标签。
 * 状态在调用间保持，便于 TagCard 复用。
 */
export function useTags() {
  const tagTypes: Ref<TagType[]> = ref([]);
  const currentVideoTags: Ref<VideoTag[]> = ref([]);
  const currentHash = ref<string>("");
  const loading = ref(false);

  // 拉取所有标签类型
  async function loadTagTypes() {
    try {
      tagTypes.value = await invoke<TagType[]>("list_tag_types");
    } catch (e) {
      console.error("[loadTagTypes] 失败:", e);
    }
  }

  // 拉取某视频的标签
  async function loadVideoTags(hash: string) {
    currentHash.value = hash;
    try {
      currentVideoTags.value = await invoke<VideoTag[]>("list_video_tags", {
        videoHash: hash,
      });
    } catch (e) {
      console.error("[loadVideoTags] 失败:", e);
    }
  }

  // 获取某标签类型的当前值（没设置则返回空）
  function getValue(typeId: number): string {
    const t = currentVideoTags.value.find((x) => x.type_id === typeId);
    return t ? t.value : "";
  }

  // 设置标签值（空字符串=清除）
  async function setValue(typeId: number, value: string) {
    if (!currentHash.value) return;
    try {
      await invoke("set_video_tag", {
        videoHash: currentHash.value,
        typeId,
        value,
      });
      await loadVideoTags(currentHash.value);
    } catch (e) {
      console.error("[setValue] 失败:", e);
    }
  }

  // 新建自定义标签类型
  async function createTagType(name: string, valueType: "enum" | "free", options: string[]) {
    try {
      await invoke("create_tag_type", { name, valueType, options });
      await loadTagTypes();
    } catch (e) {
      console.error("[createTagType] 失败:", e);
    }
  }

  // 删除标签类型
  async function deleteTagType(typeId: number) {
    try {
      await invoke("delete_tag_type", { typeId });
      await loadTagTypes();
    } catch (e) {
      console.error("[deleteTagType] 失败:", e);
    }
  }

  return {
    tagTypes,
    currentVideoTags,
    currentHash,
    loading,
    loadTagTypes,
    loadVideoTags,
    getValue,
    setValue,
    createTagType,
    deleteTagType,
  };
}
