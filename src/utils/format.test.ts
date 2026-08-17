import { describe, expect, it } from "vitest";
import { formatSize, formatTime } from "./format";

 describe("format utilities", () => {
  it("formats time boundaries", () => {
    expect(formatTime(0)).toBe("00:00");
    expect(formatTime(Number.NaN)).toBe("00:00");
    expect(formatTime(59)).toBe("00:59");
    expect(formatTime(60)).toBe("01:00");
    expect(formatTime(3600)).toBe("01:00:00");
  });

  it("formats byte units", () => {
    expect(formatSize(0)).toBe("0 B");
    expect(formatSize(1024)).toBe("1.0 KB");
    expect(formatSize(1024 * 1024)).toBe("1.0 MB");
  });
});
