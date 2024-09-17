import { Accessor, batch, createSignal, onCleanup, onMount, Setter, Show } from "solid-js";
import { UnlistenFn, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api";
import { filename, unit } from "../utils";
import styles from "./footer.module.scss";

type JobType = "uploading" | "downloading";

export default function Footer() {
  const [job, setJob] = createSignal<JobType | null>(null);
  const [finished, setFinished] = createSignal(true);

  let unlistenJobCanceled: UnlistenFn | null = null;
  let unlistenUploadError: UnlistenFn | null = null;

  onMount(async () => {
    unlistenJobCanceled = await listen("job_canceled", () => {
      setFinished(true);
    });

    unlistenUploadError = await listen("upload_error", () => {
      setFinished(true);
    });
  });

  onCleanup(() => {
    unlistenJobCanceled?.();
    unlistenUploadError?.();
  });

  return (
    <div
      class={styles.footer}
      classList={{
        [styles.expanded]: !finished(),
      }}
    >
      <UploadFooter
        isActive={() => job() === "uploading" && !finished()}
        setFinished={setFinished}
        setJob={setJob}
      />
    </div>
  );
}

type FooterComponent = {
  isActive: Accessor<boolean>;
  setFinished: Setter<boolean>;
  setJob: Setter<JobType | null>;
}

type UploadQueue = Array<{ name: string, size: number }>;

function UploadFooter({ isActive, setFinished, setJob }: FooterComponent) {
  const [queued, setQueued] = createSignal<UploadQueue>([]);
  const [current, setCurrent] = createSignal<{ index: number, progress: number } | null>(null);

  // Total size of all files in the queue
  const totalSize = () => queued().reduce((acc, { size }) => acc + size, 0);

  // Total upload progress of all files in the queue so far (including the current file)
  const totalProgress = () => queued().slice(0, current()?.index || 0).reduce((acc, { size }) => acc + size, 0) + (current()?.progress || 0);

  // Percentage of the current file's upload progress
  const percentage = () => current() !== null
    ? (queued()[current()!.index].size === 0 ? 100 : current()!.progress / queued()[current()!.index].size * 100)
    : 0;

  let unlistenExtQueue: UnlistenFn | null = null;
  let unlistenUploadProgress: UnlistenFn | null = null;
  let unlistenFileUploaded: UnlistenFn | null = null;

  onMount(async () => {
    unlistenExtQueue = await listen<Array<[string, number]>>("extend_upload_queue", ({ payload }) => {
      if (isActive()) {
        setQueued([...queued(), ...payload.map(([name, size]) => ({ name: filename(name), size }))]);
        return;
      }

      batch(() => {
        setQueued(payload.map(([name, size]) => ({ name: filename(name), size })));
        setCurrent({ index: 0, progress: 0 });
        setJob("uploading");
        setFinished(false);
      });
    });

    unlistenUploadProgress = await listen<number>("upload_progress", ({ payload }) => {
      setCurrent(({ index: current()?.index || 0, progress: payload }));
    });

    unlistenFileUploaded = await listen("file_uploaded", () => {
      const shouldFinish = queued().length === (current()?.index || 0) + 1;
      if (shouldFinish) {
        setFinished(true);
        return;
      }

      setCurrent({ index: (current()?.index || 0) + 1, progress: 0 });
    });
  });

  onCleanup(() => {
    unlistenExtQueue?.();
    unlistenUploadProgress?.();
    unlistenFileUploaded?.();
  });

  return (
    <Show when={isActive()}>
      <div class={styles.left}>
        <div class={styles.text}>
          Uploading { } {current() !== null && queued()[current()!.index].name}...
        </div>
        <div class={styles.subtext} classList={{ [styles.single]: queued().length <= 1 }}>
          <p>{unit(current()?.progress || 0)} / {current() !== null && unit(queued()[current()!.index].size)}</p>
          <div class={styles.separator} />
          <p>
            {unit(totalProgress())} / {unit(totalSize())} ({(current()?.index || 0) + 1}/{queued().length})
          </p>
        </div>
        <div class={styles.progress}>
          <div class={styles.bar} style={{ width: current() === null ? "0%" : `${percentage()}%` }} />
        </div>
      </div>
      <div class={styles.right}>
        <div class={styles.cancel} onClick={() => invoke("cancel")}>
          Cancel
        </div>
      </div>
    </Show>
  );
}