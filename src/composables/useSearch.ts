import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { VideoInfo } from "./useTags";

export type SortKey = "file_name" | "file_path" | "size_bytes" | "modified_at" | "stars" | "quality";
export type SortDir = "asc" | "desc";

interface SearchPage {
  items: VideoInfo[];
  page: number;
  page_size: number;
  total: number;
  total_pages: number;
}

export function useSearch(delay = 150) {
  const keyword = ref("");
  const selectedStars = ref<number | null>(null);
  const results = ref<VideoInfo[]>([]);
  const searching = ref(false);
  const error = ref("");
  const page = ref(1);
  const pageSize = ref(50);
  const total = ref(0);
  const totalPages = ref(0);
  const sortKey = ref<SortKey>("modified_at");
  const sortDir = ref<SortDir>("desc");
  let timer: ReturnType<typeof setTimeout> | null = null;
  let requestId = 0;

  function invalidate() {
    requestId++;
    if (timer) clearTimeout(timer);
    timer = null;
  }

  async function run(id = requestId) {
    searching.value = true;
    error.value = "";
    try {
      const data = await invoke<SearchPage>("search_videos_page", {
        request: {
          keyword: keyword.value.trim(), stars: selectedStars.value,
          sortKey: sortKey.value, sortDir: sortDir.value,
          page: page.value, pageSize: pageSize.value,
        },
      });
      if (id !== requestId) return;
      results.value = data.items;
      page.value = data.page;
      total.value = data.total;
      totalPages.value = data.total_pages;
    } catch (cause) {
      if (id === requestId) {
        results.value = [];
        total.value = 0;
        totalPages.value = 0;
        error.value = cause instanceof Error ? cause.message : String(cause);
      }
      console.error("[搜索] 失败:", cause);
    } finally {
      if (id === requestId) searching.value = false;
    }
  }

  function schedule() {
    invalidate();
    page.value = 1;
    const id = requestId;
    if (!keyword.value.trim() && selectedStars.value === null) {
      results.value = []; total.value = 0; totalPages.value = 0; searching.value = false; error.value = ""; return;
    }
    searching.value = true;
    timer = setTimeout(() => void run(id), delay);
  }

  function setKeyword(value: string) { keyword.value = value; schedule(); }
  function setStars(stars: number | null) {
    selectedStars.value = selectedStars.value === stars ? null : stars;
    schedule();
  }
  function setSort(key: SortKey) {
    if (sortKey.value === key) sortDir.value = sortDir.value === "asc" ? "desc" : "asc";
    else { sortKey.value = key; sortDir.value = "desc"; }
    schedule();
  }
  function setPage(value: number) {
    invalidate(); page.value = Math.max(1, value); void run(requestId);
  }
  function refresh() {
    invalidate();
    if (!hasFilter()) return;
    void run(requestId);
  }
  function clear() {
    invalidate(); keyword.value = ""; selectedStars.value = null; results.value = []; error.value = "";
    page.value = 1; total.value = 0; totalPages.value = 0; searching.value = false;
  }
  const hasFilter = () => keyword.value.trim().length > 0 || selectedStars.value !== null;

  return { keyword, selectedStars, results, searching, error, page, pageSize, total, totalPages,
    sortKey, sortDir, setKeyword, setStars, setSort, setPage, refresh, clear, hasFilter };
}
