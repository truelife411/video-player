import { describe, expect, it } from "vitest";
import { resolveShortcut } from "./shortcuts";

function event(key: string, extra: Partial<KeyboardEvent> = {}) {
  return { key, code: key === " " ? "Space" : "", ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, repeat: false, isComposing: false, ...extra } as KeyboardEvent;
}

describe("resolveShortcut", () => {
  it("blocks playback shortcuts in editable controls", () => {
    expect(resolveShortcut(event("s"), { hasFile: true, editable: true, interactive: false })).toBeNull();
  });
  it("keeps search and AB shortcuts", () => {
    expect(resolveShortcut(event("f", { ctrlKey: true }), { hasFile: false, editable: false, interactive: false })).toBe("toggle-search");
    expect(resolveShortcut(event("["), { hasFile: true, editable: false, interactive: false })).toBe("ab-a");
  });
});
