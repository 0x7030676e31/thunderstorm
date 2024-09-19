/// <reference types="vite/client" />

export {};

declare global {
  interface IFile {
    id: number;
    path: string;
    size: number;
    created_at: number;
    encryption_key: null | number[];
  }

  interface ISettings {
    token: string;
    channel: string;
    guild: string;
    do_encrypt: boolean;
    do_checksum: boolean;
  }

  interface IError {
    job: string;
    type: string;
    message: string;
  }
}
