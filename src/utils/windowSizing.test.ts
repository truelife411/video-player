import { describe, expect, it } from "vitest";
import { calculateWindowSize } from "./windowSizing";

describe("calculateWindowSize", () => {
  it("does not scale the fixed UI height", () => {
    expect(calculateWindowSize({ videoWidth: 1000, videoHeight: 500, scale: 0.5, uiHeight: 100, workWidth: 2000, workHeight: 1200, policy: "video" }))
      .toEqual({ action: "resize", width: 640, height: 400 });
  });

  it("fits a 4K video into the largest normal window", () => {
    expect(calculateWindowSize({ videoWidth: 3840, videoHeight: 2160, scale: 1, uiHeight: 172, workWidth: 1920, workHeight: 1040, policy: "video" }))
      .toEqual({ action: "resize", width: 1543, height: 1040 });
  });

  it("keeps portrait and ultrawide videos proportional", () => {
    expect(calculateWindowSize({ videoWidth: 1080, videoHeight: 1920, scale: 1, uiHeight: 100, workWidth: 1920, workHeight: 1000, policy: "video" }))
      .toEqual({ action: "resize", width: 640, height: 1000 });
    expect(calculateWindowSize({ videoWidth: 7680, videoHeight: 1080, scale: 1, uiHeight: 100, workWidth: 1920, workHeight: 1000, policy: "video" }))
      .toEqual({ action: "resize", width: 1920, height: 400 });
  });

  it("caps a large user scale without maximizing", () => {
    const result = calculateWindowSize({ videoWidth: 1920, videoHeight: 1080, scale: 3, uiHeight: 100, workWidth: 1600, workHeight: 900, policy: "video" });
    expect(result).toEqual({ action: "resize", width: 1422, height: 900 });
  });

  it("lets fit enlarge a small video", () => {
    expect(calculateWindowSize({ videoWidth: 640, videoHeight: 360, scale: 0.5, uiHeight: 100, workWidth: 1920, workHeight: 1000, policy: "fit" }))
      .toEqual({ action: "resize", width: 1600, height: 1000 });
  });

  it("never exceeds a work area smaller than the minimum window", () => {
    expect(calculateWindowSize({ videoWidth: 1920, videoHeight: 1080, scale: 1, uiHeight: 172, workWidth: 500, workHeight: 300, policy: "video" }))
      .toEqual({ action: "resize", width: 500, height: 300 });
  });

  it("leaves room for the native title bar and borders", () => {
    const frameWidth = 16;
    const frameHeight = 39;
    const result = calculateWindowSize({ videoWidth: 3840, videoHeight: 2160, scale: 1, uiHeight: 140, workWidth: 1920 - frameWidth, workHeight: 1040 - frameHeight, policy: "video" });
    expect(result).toEqual({ action: "resize", width: 1530, height: 1001 });
    if (result.action === "resize") {
      expect(result.width + frameWidth).toBeLessThanOrEqual(1920);
      expect(result.height + frameHeight).toBeLessThanOrEqual(1040);
    }
  });

  it("supports keep and explicit maximize", () => {
    const base = { videoWidth: 1920, videoHeight: 1080, scale: 1, uiHeight: 100, workWidth: 1920, workHeight: 1080 };
    expect(calculateWindowSize({ ...base, policy: "keep" })).toEqual({ action: "keep" });
    expect(calculateWindowSize({ ...base, policy: "maximize" })).toEqual({ action: "maximize" });
  });
});
