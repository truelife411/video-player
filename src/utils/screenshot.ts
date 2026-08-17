const RESERVED = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i;
let lastStamp = "";
let sequence = 0;

export function sanitizeWindowsFileStem(value: string, maxLength = 80) {
  let stem = value.replace(/\.[^.]+$/, "");
  stem = stem.replace(/[<>:"/\\|?*\u0000-\u001f]/g, "_").replace(/[ .]+$/g, "").trim();
  if (!stem) stem = "screenshot";
  if (RESERVED.test(stem)) stem = `_${stem}`;
  return Array.from(stem).slice(0, maxLength).join("");
}

function pad(value: number, length = 2) {
  return String(value).padStart(length, "0");
}

function positionStamp(seconds: number) {
  const milliseconds = Math.max(0, Math.floor(seconds * 1000));
  const hours = Math.floor(milliseconds / 3_600_000);
  const minutes = Math.floor((milliseconds % 3_600_000) / 60_000);
  const secs = Math.floor((milliseconds % 60_000) / 1000);
  return `${pad(hours)}-${pad(minutes)}-${pad(secs)}.${pad(milliseconds % 1000, 3)}`;
}

export function buildScreenshotFileName(
  videoFileName: string,
  positionSeconds: number,
  includeSubtitles: boolean,
  date = new Date(),
) {
  const stamp = `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}-${pad(date.getMilliseconds(), 3)}`;
  if (stamp === lastStamp) sequence += 1;
  else {
    lastStamp = stamp;
    sequence = 0;
  }
  const suffix = sequence ? `-${pad(sequence)}` : "";
  const mode = includeSubtitles ? "sub" : "video";
  return `${sanitizeWindowsFileStem(videoFileName)}_${positionStamp(positionSeconds)}_${stamp}${suffix}_${mode}.png`;
}
