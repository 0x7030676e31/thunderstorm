import { For, Accessor, createEffect, createSignal, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/tauri";
import { File, PendingFile } from "./file";
import styles from "../styles/content.module.scss";

type Props = {
  files: Accessor<File[]>,
  pendingFiles: Accessor<PendingFile[]>,
  select: (idx: number | null) => void,
  query: Accessor<string>,
}

export default function Content({ files, pendingFiles, select, query }: Props) {
  const [ filteredFiles, setFilteredFiles ] = createSignal<File[] | null>(null);

  createEffect(async () => {
    select(null);
    if (query() === "") {
      setFilteredFiles(null);
      return;
    }

    const filtered = await invoke<string>("search", { query: query() }).then(state => JSON.parse(state));
    setFilteredFiles(filtered);
  });
  
  return (
    <div class={styles.content}>
      <For each={pendingFiles()}>
        {file => <PendingFile {...file} />}
      </For>
      <For each={filteredFiles() ?? files()}>
        {(file, idx) => <File {...file} select={() => select(idx())} />}
      </For>
      <Show when={!filteredFiles() && !files().length && !pendingFiles().length}>
        {fallback({ searching: query() !== "" })}
      </Show>
    </div>
  )
}

function fallback({ searching }: { searching: boolean }) {
  return (
    <div class={styles.fallback}>
      <h1>(╯°□°)╯︵ ┻━┻</h1>
      <p>{searching ? "No files match your search" : "Drop files here to upload"}</p>
    </div>
  )
}