import { type } from "@tauri-apps/api/os";

let fileSeparator = "/";
type().then((osType) => (fileSeparator = osType === "Windows_NT" ? "\\" : "/"));

export function unit(size: number) {
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
  let unit = 0;

  while (size >= 1024) {
    size /= 1024;
    unit++;
  }

  return `${unit === 0 ? size.toString() : size.toFixed(2)} ${units[unit]}`;
}

export function filename(file: string) {
  return file.split(fileSeparator).pop()?.trim() || file;
}
