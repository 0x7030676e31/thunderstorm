/// <reference types="vite/client" />

export {};

declare global {
  interface IFile {
    id: number;
    path: string;
    name: string | null;
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
    download_location: string;
  }

  interface IError {
    job: string;
    type: string;
    message: string;
  }
}
