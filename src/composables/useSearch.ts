import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { VideoInfo } from "./useTags";

/**
 * 搜索：两种模式
 *   1) 关键词搜索（debounce 150ms）：匹配文件名或任何标签值
 *   2) 星级筛选：点击星级按钮后，纯按星级查（无关键词）
 * 两种模式互斥——切到星级时清空关键词，切到关键词时清空星级。
 */
export function useSearch(delay = 150) {
  const keyword = ref("");
  const selectedStars = ref<number | null>(null); // null=未选星级
  const results = ref<VideoInfo[]>([]);
  const searching = ref(false);

  let timer: ReturnType<typeof setTimeout> | null = null;

  // 关键词搜索（切到关键词模式）
  function setKeyword(k: string) {
    selectedStars.value = null; // 切到关键词模式，清空星级
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

  // 星级筛选（切到星级模式）
  async function setStars(n: number | null) {
    if (timer) clearTimeout(timer);
    // 同一星级再点 = 取消
    if (selectedStars.value === n) {
      selectedStars.value = null;
      keyword.value = "";
      results.value = [];
      return;
    }
    selectedStars.value = n;
    keyword.value = ""; // 纯星级，清空关键词
    if (n === null) {
      results.value = [];
      return;
    }
    searching.value = true;
    try {
      results.value = await invoke<VideoInfo[]>("list_videos_by_stars", { stars: n });
    } catch (e) {
      console.error("[星级筛选] 失败:", e);
      results.value = [];
    } finally {
      searching.value = false;
    }
  }

  function clear() {
    keyword.value = "";
    selectedStars.value = null;
    results.value = [];
    searching.value = false;
    if (timer) clearTimeout(timer);
  }

  // 是否处于「有筛选条件」状态（用于空状态提示文案）
  const hasFilter = () => keyword.value.trim().length > 0 || selectedStars.value !== null;

  return { keyword, selectedStars, results, searching, setKeyword, setStars, clear, hasFilter };
}
