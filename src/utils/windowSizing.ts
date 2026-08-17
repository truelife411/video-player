export type WindowSizePolicy = "video" | "keep" | "fit" | "maximize";

export interface WindowSizingInput {
  videoWidth: number;
  videoHeight: number;
  scale: number;
  uiHeight: number;
  workWidth: number;
  workHeight: number;
  minWidth?: number;
  minHeight?: number;
  policy: WindowSizePolicy;
}

export type WindowSizingResult =
  | { action: "keep" }
  | { action: "maximize" }
  | { action: "resize"; width: number; height: number };

export function calculateWindowSize(input: WindowSizingInput): WindowSizingResult {
  if (input.policy === "keep") return { action: "keep" };
  if (input.policy === "maximize") return { action: "maximize" };

  const workWidth = Math.max(1, Math.floor(input.workWidth));
  const workHeight = Math.max(1, Math.floor(input.workHeight));
  const uiHeight = Math.max(0, Math.min(workHeight - 1, Math.round(input.uiHeight)));
  const availableVideoHeight = Math.max(1, workHeight - uiHeight);
  const requestedScale = input.policy === "fit" ? Number.POSITIVE_INFINITY : Math.max(0.01, input.scale);
  const actualScale = Math.min(
    requestedScale,
    workWidth / input.videoWidth,
    availableVideoHeight / input.videoHeight
  );
  const videoWidth = Math.max(1, Math.floor(input.videoWidth * actualScale));
  const videoHeight = Math.max(1, Math.floor(input.videoHeight * actualScale));
  const contentWidth = Math.min(workWidth, videoWidth);
  const contentHeight = Math.min(workHeight, videoHeight + uiHeight);
  const minWidth = Math.min(workWidth, input.minWidth ?? 640);
  const minHeight = Math.min(workHeight, input.minHeight ?? 400);

  return {
    action: "resize",
    width: Math.min(workWidth, Math.max(minWidth, contentWidth)),
    height: Math.min(workHeight, Math.max(minHeight, contentHeight)),
  };
}
