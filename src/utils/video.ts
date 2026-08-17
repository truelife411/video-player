export const VIDEO_EXTENSIONS = [
  "mkv", "mp4", "avi", "mov", "webm", "flv", "ts", "m4v", "wmv", "mpg", "mpeg", "vob",
] as const;

export const SUBTITLE_EXTENSIONS = ["srt", "ass", "ssa", "sub", "vtt", "lrc"] as const;

export function extensionOf(path: string) {
  return path.split(".").pop()?.toLowerCase() ?? "";
}

export function leadingStarCount(path: string) {
  const match = (path.split(/[\\/]/).pop() || "").match(/^★+/);
  return match ? Math.min(7, match[0].length) : 0;
}

export function qualityFromHeight(height: number) {
  if (height >= 2160) return "4K";
  if (height >= 1080) return "1080p";
  if (height >= 720) return "720p";
  return "480p";
}
