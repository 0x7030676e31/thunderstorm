import { batch, createSignal, onCleanup, onMount } from "solid-js";
import { UnlistenFn, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api";
import { unit } from "../utils";
import styles from "./footer.module.scss";

export default function Footer() {
  const [ queued, setQueued ] = createSignal<Array<{ name: string, size: number }>>([]);
  const [ current, setCurrent ] = createSignal<{ index: number, progress: number } | null>(null);
  const [ progress, setProgress ] = createSignal(0);
  const [ finished, setFinished ] = createSignal(true);

  const totalSize = () => queued().reduce((acc, { size }) => acc + size, 0);
  const totalProgress = () => progress() + (current()?.progress || 0);
  const progressPercentage = () => current() !== null
    ? (queued()[current()!.index].size === 0 ? 100 : current()!.progress / queued()[current()!.index].size * 100)
    : 0;

  let unlistenQueue: UnlistenFn | null = null;
  let unlistenProgress: UnlistenFn | null = null;
  let unlistenUploaded: UnlistenFn | null = null;
  let unlistenCancel: UnlistenFn | null = null;
  let unlistenUploadError: UnlistenFn | null = null;

  onMount(async () => {
    unlistenQueue = await listen<Array<[string, number]>>("queue", data => {
      batch(() => {
        if (!finished()) {
          setQueued(queued => [ ...queued, ...data.payload.map(([ name, size ]) => ({ name: name.split("/").pop()?.trim() || name, size }))]);
          return;
        }

        setFinished(false);
        setProgress(0);
        setCurrent({ index: 0, progress: 0 });
        setQueued(data.payload.map(([ name, size ]) => ({ name: name.split("/").pop()?.trim() || name, size })));
      });
    });

    unlistenProgress = await listen<number>("progress", data => {
      batch(() => {
        setCurrent({ index: current()?.index || 0, progress: data.payload });
      });
    });

    unlistenUploaded = await listen("uploaded", () => {
      batch(() => {
        if (queued().length === (current()?.index || 0) + 1) {
          setFinished(true);
          return;
        }

        setProgress(progress => progress + (current()?.progress || 0));
        setCurrent({ index: (current()?.index || 0) + 1, progress: 0 });
      });
    });

    unlistenCancel = await listen("cancel", () => {
      setFinished(true);
    });

    unlistenUploadError = await listen("upload_error", () => {
      setFinished(true);
    });
  });

  onCleanup(() => {
    unlistenQueue?.();
    unlistenProgress?.();
    unlistenUploaded?.();
    unlistenCancel?.();
  });

  return (
    <div
      class={styles.footer}
      classList={{
        [styles.expanded]: !finished(),
      }}
    >
      <div class={styles.left}>
        <div class={styles.text}>
          Uploading {current() !== null && queued()[current()!.index].name}...
        </div>
        <div class={styles.subtext} classList={{ [styles.single]: queued().length <= 1 }}>
          <p>{unit(current()?.progress || 0)} / {current() !== null && unit(queued()[current()!.index].size)}</p>
          <div class={styles.separator} />
          <p>
            {unit(totalProgress())} / {unit(totalSize())} ({(current()?.index || 0) + 1}/{queued().length})
          </p>
        </div>
        <div class={styles.progress}>
          <div class={styles.bar} style={{ width: current() === null ? "0%" : `${progressPercentage()}%` }} />
        </div>
      </div>
      <div class={styles.right}>
        <div class={styles.cancel} onClick={() => invoke("cancel")}>
          Cancel
        </div>
      </div>
    </div>
  );
}