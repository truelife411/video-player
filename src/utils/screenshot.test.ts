import { describe, expect, it } from "vitest";
import { buildScreenshotFileName, sanitizeWindowsFileStem } from "./screenshot";

describe("screenshot naming", () => {
  it("sanitizes Windows names", () => {
    expect(sanitizeWindowsFileStem('CON.mp4')).toBe("_CON");
    expect(sanitizeWindowsFileStem('a:b?.mp4')).toBe("a_b_");
  });
  it("includes position, milliseconds and mode", () => {
    const name = buildScreenshotFileName("电影.mkv", 65.432, false, new Date(2026, 6, 18, 12, 3, 4, 5));
    expect(name).toContain("电影_00-01-05.432_20260718-120304-005");
    expect(name.endsWith("_video.png")).toBe(true);
  });
});
