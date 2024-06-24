/// <reference types="vite/client" />

export {};

declare global {
  interface IFile {
    id: number,
    name: string,
    size: number,
    created: number,
  }

  interface ISettings {
    token: string,
    channel: string,
    guild: string,
  }
}