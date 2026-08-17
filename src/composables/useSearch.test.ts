import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { useSearch } from "./useSearch";

const page = (items: unknown[]) => ({ items, page: 1, page_size: 50, total: items.length, total_pages: 1 });

describe("useSearch", () => {
  beforeEach(() => { vi.useFakeTimers(); invoke.mockReset(); });

  it("debounces keyword requests", async () => {
    invoke.mockResolvedValue(page([]));
    const search = useSearch();
    search.setKeyword("a"); search.setKeyword("ab");
    await vi.advanceTimersByTimeAsync(149);
    expect(invoke).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(invoke).toHaveBeenCalledWith("search_videos_page", {
      request: {
        keyword: "ab",
        stars: null,
        sortKey: "modified_at",
        sortDir: "desc",
        page: 1,
        pageSize: 50,
      },
    });
  });

  it("exposes search failures instead of showing an empty result", async () => {
    invoke.mockRejectedValue(new Error("invalid args"));
    const search = useSearch(0);
    search.setStars(4);
    await vi.advanceTimersByTimeAsync(0);
    await Promise.resolve();
    expect(search.results.value).toEqual([]);
    expect(search.error.value).toContain("invalid args");
  });

  it("keeps star filter and ignores an older response", async () => {
    let resolveOld!: (value: unknown) => void;
    invoke.mockImplementationOnce(() => new Promise((resolve) => (resolveOld = resolve)))
      .mockResolvedValueOnce(page([{ hash: "new" }]));
    const search = useSearch(0);
    search.setStars(5);
    search.setKeyword("old"); await vi.advanceTimersByTimeAsync(0);
    search.setKeyword("new"); await vi.advanceTimersByTimeAsync(0); await Promise.resolve();
    resolveOld(page([{ hash: "old" }])); await Promise.resolve();
    expect(search.selectedStars.value).toBe(5);
    expect(search.results.value).toEqual([{ hash: "new" }]);
  });
});
