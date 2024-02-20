import { AiFillFileText } from 'solid-icons/ai'
import styles from "../styles/file.module.scss";
import { createEffect, createSignal, onCleanup } from 'solid-js';
import { UnlistenFn, listen } from '@tauri-apps/api/event';

const units = [ "B", "KiB", "MiB", "GiB", "TiB" ];
function unit(bytes: number) {
  let i = 0;
  while (bytes > 1024) {
    bytes /= 1024;
    i++;
  }
  return `${+bytes.toFixed(2)} ${units[i]}`;
}

export function File({ name, size, created, select }: Readonly<File> & { select: () => void }) {
  return (
    <div class={`${styles.file} file`} onClick={select}>
      <AiFillFileText />
      <div class={styles.name}>{name}</div>
      <div class={styles.size}>{unit(size)}</div> 
      <div class={styles.created}>{new Date(created).toLocaleString()}</div>
    </div>
  )
}

function time(seconds: number) {
  const s = seconds % 60;
  const m = Math.floor(seconds / 60) % 60;

  return `(${m ? m + "m " : ""}${s}s)`;
}

export function PendingFile({ name, size }: PendingFile) {
  const [ progress, setProgress ] = createSignal<number>(0);
  const [ count, setCount ] = createSignal<number>(0);
  let listener: UnlistenFn | null = null;
  let interval: number | null = null;
  
  createEffect(() => {
    if (size !== null && !listener) {
      listen<number>("progress", async ({ payload }) => { setProgress(payload) }).then(unlisten => listener = unlisten);
      interval = setInterval(() => setCount(count => count + 1), 1000);
    }
  });

  onCleanup(() => {
    listener?.();
    if (interval) clearInterval(interval);
  });

  const taken = () => size ? time(count()) : null;

  return (
    <div class={`${styles.file} ${styles.pendingFile}`}>
      <AiFillFileText />
      <div class={styles.name}>{name}</div>
      <div class={styles.size}>{size ? progress() === size ? "Finalizing..." : (+(progress() / size * 100).toFixed(2)).toString() + "% " + taken() : "Pending"}</div>
    </div>
  )
}

