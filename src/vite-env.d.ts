/// <reference types="vite/client" />

export {};

declare global {
  interface IFile {
    id: number;
    path: string;
    size: number;
    created_at: number;
  }

  interface ISettings {
    token: string;
    channel: string;
    guild: string;
  }

  interface IError {
    job: string;
    type: string;
    message: string;
  }
}
