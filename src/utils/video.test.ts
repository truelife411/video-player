import { describe, expect, it } from "vitest";
import { leadingStarCount, qualityFromHeight } from "./video";

describe("video utilities", () => {
  it("counts and caps leading stars", () => {
    expect(leadingStarCount("C:/videos/★★★name.mp4")).toBe(3);
    expect(leadingStarCount("★★★★★★★★name.mp4")).toBe(7);
    expect(leadingStarCount("name★.mp4")).toBe(0);
  });

  it("maps height to preset quality", () => {
    expect(qualityFromHeight(480)).toBe("480p");
    expect(qualityFromHeight(720)).toBe("720p");
    expect(qualityFromHeight(1080)).toBe("1080p");
    expect(qualityFromHeight(2160)).toBe("4K");
  });
});
