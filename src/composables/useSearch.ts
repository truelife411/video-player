import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { VideoInfo } from "./useTags";

/**
 * 搜索：实时筛选（debounce 150ms），调用 Rust search_videos
 * 匹配文件名或任何标签值
 */
export function useSearch(delay = 150) {
  const keyword = ref("");
  const results = ref<VideoInfo[]>([]);
  const searching = ref(false);

  let timer: ReturnType<typeof setTimeout> | null = null;

  function setKeyword(k: string) {
    keyword.value = k;
    if (timer) clearTimeout(timer);
    if (!k.trim()) {
      results.value = [];
      searching.value = false;
      return;
    }
    searching.value = true;
    timer = setTimeout(async () => {
      try {
        results.value = await invoke<VideoInfo[]>("search_videos", { keyword: k.trim() });
      } catch (e) {
        console.error("[搜索] 失败:", e);
        results.value = [];
      } finally {
        searching.value = false;
      }
    }, delay);
  }

  function clear() {
    keyword.value = "";
    results.value = [];
    if (timer) clearTimeout(timer);
  }

  return { keyword, results, searching, setKeyword, clear };
}
