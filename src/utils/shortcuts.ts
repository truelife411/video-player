export type ShortcutAction =
  | "toggle-play" | "seek-back" | "seek-forward" | "volume-up" | "volume-down"
  | "toggle-mute" | "toggle-fullscreen" | "toggle-search" | "toggle-tag"
  | "screenshot" | "toggle-subtitles" | "frame-back" | "frame-forward"
  | "rotate-cw" | "rotate-ccw" | "flip-h" | "flip-v"
  | "ab-a" | "ab-b" | "ab-clear" | "escape";

export interface ShortcutContext {
  hasFile: boolean;
  editable: boolean;
  interactive: boolean;
}

const REPEATABLE = new Set(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"]);

export function resolveShortcut(event: KeyboardEvent, context: ShortcutContext): ShortcutAction | null {
  if (event.isComposing) return null;
  if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === "f") return "toggle-search";
  if (event.key === "Escape") return "escape";
  if (context.editable || context.interactive) return null;
  if (event.ctrlKey || event.metaKey || event.altKey) return null;
  if (event.repeat && !REPEATABLE.has(event.key)) return null;

  const key = event.key.toLowerCase();
  if (key === "f") return "toggle-fullscreen";
  if (!context.hasFile) return null;
  if (event.code === "Space") return "toggle-play";
  if (event.key === "ArrowLeft") return "seek-back";
  if (event.key === "ArrowRight") return "seek-forward";
  if (event.key === "ArrowUp") return "volume-up";
  if (event.key === "ArrowDown") return "volume-down";
  if (key === "m") return "toggle-mute";
  if (key === "t") return "toggle-tag";
  if (key === "s") return "screenshot";
  if (key === "c") return "toggle-subtitles";
  if (key === ",") return "frame-back";
  if (key === ".") return "frame-forward";
  if (key === "r") return event.shiftKey ? "rotate-ccw" : "rotate-cw";
  if (key === "h") return "flip-h";
  if (key === "v") return "flip-v";
  if (event.key === "[") return "ab-a";
  if (event.key === "]") return "ab-b";
  if (event.key === "\\") return "ab-clear";
  return null;
}

export function shortcutContext(target: EventTarget | null, hasFile: boolean): ShortcutContext {
  const element = target instanceof Element ? target : null;
  return {
    hasFile,
    editable: !!element?.closest("input,textarea,select,[contenteditable='true']"),
    interactive: !!element?.closest("button,a,[role='button'],[role='switch']"),
  };
}
