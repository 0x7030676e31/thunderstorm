import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { UnlistenFn, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api";
import { unit } from "../utils";
import styles from "./content.module.scss";

type Props = {
  setSelected: any // todo;
  selected: number[];
  files: IFile[];
}

const fmt = new Intl.DateTimeFormat(undefined, {
  weekday: "short",
  year: "numeric",
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "numeric",
  second: "numeric",
});

export default function Content(props: Props) {
  const [ hovering, setHovering ] = createSignal(false);
  
  let unlistenDrop: UnlistenFn | null = null;
  let unlistenDropHover: UnlistenFn | null = null;
  let unlistenDropCancelled: UnlistenFn | null = null;

  onMount(async () => {
    unlistenDrop = await listen("tauri://file-drop", async data => {
      setHovering(false);
      await invoke("upload_files", {
        files: data.payload,
      });
    });

    unlistenDropHover = await listen("tauri://file-drop-hover", () => {
      setHovering(true);
    });

    unlistenDropCancelled = await listen("tauri://tauri://file-drop-cancelled", () => {
      setHovering(false);
    });
  });

  onCleanup(() => {
    unlistenDrop?.();
    unlistenDropHover?.();
    unlistenDropCancelled?.();
  });

  return (
    <div class={styles.content}>
      <div class={styles.files}>
        <For each={props.files}>
          {file => (
            <div class={styles.file}>
              <div>
                {file.name.split("/").pop()?.trim() || file.name}
              </div>
              <div class={styles.size}>
                {unit(file.size)}
              </div>
              <div>
                {fmt.format(new Date(file.created_at * 1000))}
              </div>
            </div>
          )}
        </For>
      </div>
      <div class={styles.overlay} classList={{ [styles.hovering]: hovering() }} />
      <Show when={props.files.length === 0}>
        <Fallback />
      </Show>
    </div>
  );
}

function Fallback() {
  return (
    <div class={styles.fallback}>
      <h1>(╯°□°)╯︵ ┻━┻</h1>
      <p>Drop files here to upload</p>
    </div>
  );
}