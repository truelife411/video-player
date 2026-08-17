import { describe, expect, it, vi } from "vitest";
import { LoadGate } from "./loadGate";

describe("LoadGate", () => {
  it("requires start-file before completing a load", async () => {
    const gate = new LoadGate();
    const pending = gate.wait(1, "B.mp4", 1000);
    expect(gate.handle("file-loaded", "A.mp4")).toBe(false);
    gate.handle("start-file");
    expect(gate.handle("file-loaded", "B.mp4")).toBe(true);
    await expect(pending).resolves.toBeUndefined();
  });

  it("cancels the previous waiter when a new load starts", async () => {
    const gate = new LoadGate();
    const first = gate.wait(1, "A.mp4", 1000);
    const second = gate.wait(2, "B.mp4", 1000);
    await expect(first).rejects.toThrow("新的打开请求");
    gate.handle("start-file");
    expect(gate.handle("file-loaded", "A.mp4")).toBe(false);
    gate.handle("file-loaded", "B.mp4");
    await expect(second).resolves.toBeUndefined();
  });

  it("cleans up after timeout", async () => {
    vi.useFakeTimers();
    const gate = new LoadGate();
    const pending = gate.wait(1, "A.mp4", 100);
    const assertion = expect(pending).rejects.toThrow("打开视频超时");
    await vi.advanceTimersByTimeAsync(100);
    await assertion;
    expect(gate.handle("start-file")).toBe(false);
    vi.useRealTimers();
  });
});
