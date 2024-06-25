import { batch, createSignal, onCleanup, onMount } from "solid-js";
import { UnlistenFn, listen } from "@tauri-apps/api/event";
import styles from "./footer.module.scss";

function unit(size: number) {
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
  let unit = 0;

  while (size >= 1024) {
    size /= 1024;
    unit++;
  }

  return `${size.toFixed(2)} ${units[unit]}`;
}

export default function Footer() {
  const [ queued, setQueued ] = createSignal<Array<{ name: string, size: number }>>([]);
  const [ current, setCurrent ] = createSignal<{ index: number, progress: number } | null>(null);
  const [ progress, setProgress ] = createSignal(0);
  const [ finished, setFinished ] = createSignal(true);

  let unlistenQueue: UnlistenFn | null = null;
  let unlistenProgress: UnlistenFn | null = null;
  let unlistenUploaded: UnlistenFn | null = null;

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

      console.log("queue");
    });

    unlistenProgress = await listen<number>("progress", data => {
      batch(() => {
        setCurrent({ index: current()?.index || 0, progress: data.payload });
      });

      console.log("progress");
    });

    unlistenUploaded = await listen("uploaded", () => {
      batch(() => {
        if (queued().length !== (current()?.index || NaN) + 1) {
          setProgress(progress => progress + (current()?.progress || 0));
          setCurrent({ index: (current()?.index || 0) + 1, progress: 0 });
          return;
        }

        setCurrent({ index: (current()?.index || 0) + 1, progress: current()?.progress || 0 });
        setFinished(true);
      });

      console.log("uploaded");
    });
  });

  onCleanup(() => {
    unlistenQueue?.();
    unlistenProgress?.();
    unlistenUploaded?.();
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
          <p>{unit(progress() + (current()?.progress || 0))} / {unit(queued().reduce((acc, { size }) => acc + size, 0))} ({current()?.index || 0}/{queued().length})</p>
        </div>
        <div class={styles.progress}>
          <div class={styles.bar} style={{ width: current() === null ? "0%" : `${queued()[current()!.index].size === 0 ? 100 : current()!.progress / queued()[current()!.index].size * 100}%` }} />
        </div>
      </div>
      <div class={styles.right}>
        <div class={styles.cancel}>
          Cancel
        </div>
      </div>
    </div>
  );
}