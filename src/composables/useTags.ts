import { ref, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface TagOption {
  id: number;
  value: string;
  sort_order: number;
}

export interface TagType {
  id: number;
  name: string;
  value_type: "enum" | "free";
  is_preset: boolean;
  system_key: "stars" | "quality" | null;
  is_multi: boolean;
  sort_order: number;
  options: TagOption[];
}

export interface VideoTag {
  type_id: number;
  type_name: string;
  value_type: "enum" | "free";
  system_key: string | null;
  is_multi: boolean;
  values: string[];
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
  stars: number;
  quality: string;
}

export function useTags() {
  const tagTypes: Ref<TagType[]> = ref([]);
  const currentVideoTags: Ref<VideoTag[]> = ref([]);
  const currentHash = ref("");
  const loading = ref(false);
  const error = ref("");
  let loadId = 0;

  async function loadTagTypes() {
    loading.value = true;
    try {
      tagTypes.value = await invoke<TagType[]>("list_tag_types");
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function loadVideoTags(hash: string) {
    const id = ++loadId;
    currentHash.value = hash;
    currentVideoTags.value = [];
    error.value = "";
    try {
      const result = await invoke<VideoTag[]>("list_video_tags", { videoHash: hash });
      if (id === loadId && currentHash.value === hash) currentVideoTags.value = result;
    } catch (cause) {
      error.value = String(cause);
      throw cause;
    }
  }

  function getValues(typeId: number): string[] {
    return currentVideoTags.value.find((tag) => tag.type_id === typeId)?.values ?? [];
  }

  function getValue(typeId: number) {
    return getValues(typeId)[0] ?? "";
  }

  async function setValues(typeId: number, values: string[]) {
    if (!currentHash.value) return;
    await invoke("set_video_tag_values", { videoHash: currentHash.value, typeId, values });
    await loadVideoTags(currentHash.value);
  }

  async function setValue(typeId: number, value: string) {
    await setValues(typeId, value ? [value] : []);
  }

  async function createTagType(name: string, valueType: "enum" | "free", options: string[], isMulti = false) {
    await invoke("create_tag_type", { name, valueType, options, isMulti });
    await loadTagTypes();
  }

  async function updateTagType(typeId: number, name: string, isMulti: boolean) {
    await invoke("update_tag_type", { typeId, name, isMulti });
    await loadTagTypes();
  }

  async function deleteTagType(typeId: number) {
    await invoke("delete_tag_type", { typeId });
    await loadTagTypes();
  }

  async function ensurePresets() {
    await invoke("ensure_presets");
  }

  return {
    tagTypes, currentVideoTags, currentHash, loading, error,
    loadTagTypes, loadVideoTags, getValues, getValue, setValues, setValue,
    createTagType, updateTagType, deleteTagType, ensurePresets,
  };
}
