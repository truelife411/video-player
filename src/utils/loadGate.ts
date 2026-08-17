export type LoadEvent = "start-file" | "file-loaded" | "error";

interface PendingLoad {
  token: number;
  expectedPath: string;
  started: boolean;
  resolve: () => void;
  reject: (error: Error) => void;
  timeoutId: ReturnType<typeof setTimeout>;
}

export class LoadGate {
  private pending: PendingLoad | null = null;

  wait(token: number, expectedPath: string, timeoutMs: number): Promise<void> {
    this.cancel(new Error("已被新的打开请求取消"));
    return new Promise((resolve, reject) => {
      const timeoutId = setTimeout(() => {
        if (this.pending?.token !== token) return;
        this.pending = null;
        reject(new Error("打开视频超时"));
      }, timeoutMs);
      this.pending = { token, expectedPath, started: false, resolve, reject, timeoutId };
    });
  }

  handle(event: LoadEvent, currentPath?: string, error?: Error): boolean {
    const pending = this.pending;
    if (!pending) return false;
    if (event === "start-file") {
      pending.started = true;
      return true;
    }
    if (!pending.started || !currentPath || currentPath !== pending.expectedPath) return false;
    this.pending = null;
    clearTimeout(pending.timeoutId);
    if (event === "file-loaded") pending.resolve();
    else pending.reject(error ?? new Error("无法播放该文件"));
    return true;
  }

  cancel(error = new Error("打开请求已取消")) {
    const pending = this.pending;
    if (!pending) return;
    this.pending = null;
    clearTimeout(pending.timeoutId);
    pending.reject(error);
  }
}
