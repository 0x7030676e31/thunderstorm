export {};

declare global {
  interface File {
    name: string,
    size: number,
    created: number,
  }

  interface PendingFile {
    name: string,
    size: number | null,
  }
}
