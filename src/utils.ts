export function unit(size: number) {
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
  let unit = 0;

  while (size >= 1024) {
    size /= 1024;
    unit++;
  }

  return `${size.toFixed(2)} ${units[unit]}`;
}