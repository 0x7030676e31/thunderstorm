import {
  AiOutlineFileExcel,
  AiOutlineFileGif,
  AiOutlineFileImage,
  AiOutlineFileMarkdown,
  AiOutlineFilePdf,
  AiOutlineFilePpt,
  AiOutlineFileText,
  AiOutlineFileWord,
  AiOutlineFileZip
} from "solid-icons/ai";

import { Accessor, For, Match, Setter, Show, Switch, createSignal, onCleanup, onMount } from "solid-js";
import { UnlistenFn, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api";
import { unit, filename } from "../utils";
import styles from "./content.module.scss";

type Props = {
  setSelected: Setter<number[]>;
  selected: Accessor<number[]>;
  files: Accessor<IFile[]>;
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

export default function Content({ setSelected, selected, files }: Props) {
  const [hovering, setHovering] = createSignal(false);

  const filelist = () => structuredClone(files()).reverse();

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

    unlistenDropCancelled = await listen("tauri://file-drop-cancelled", () => {
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
      <div class={styles.overlay} classList={{ [styles.hovering]: hovering() }} />

      <Show when={files().length === 0}>
        <Fallback />
      </Show>

      <div class={styles.files}>
        <For each={filelist()}>
          {file => <File
            {...file}
            selected={() => selected().includes(file.id)}
            onClick={() => setSelected(selected => {
              if (selected.includes(file.id)) {
                return selected.filter(id => id !== file.id);
              } else {
                return [...selected, file.id];
              }
            })}
          />}
        </For>
      </div>
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

type FileProps = {
  selected: Accessor<boolean>;
  onClick: () => void;
};

function File({ selected, onClick, path, size, created_at }: IFile & FileProps) {
  return (
    <div
      class={styles.file}
      classList={{ [styles.selected]: selected() }}
      onClick={onClick}
    >
      <div class={styles.icon}>
        <FileIcon filename={filename(path)} />
      </div>
      <div class={styles.name}>
        {filename(path)}
      </div>
      <div class={styles.size}>
        {unit(size)}
      </div>
      <div>
        {fmt.format(new Date(created_at * 1000))}
      </div>
    </div>
  );
}

const SPREADSHEET_EXTS = ["csv", "xls", "xlsx"];
const GIF_EXTS = ["gif"];
const IMAGE_EXTS = ["png", "jpg", "jpeg", "webp", "mp4", "webm", "mkv", "avi", "mov", "wmv", "flv"];
const MARKDOWN_EXTS = ["md"];
const PDF_EXTS = ["pdf"];
const PRESENTATION_EXTS = ["ppt", "pptx"];
const DOCUMENT_EXTS = ["doc", "docx"];

// Pure insanity
const ARCHIVE_EXTS = ["7z", "s7z", "aar", "ace", "afa", "alz", "apk", "arc", "ark", "cdx", "arj", "b1", "b6z", "ba", "bh", "cab", "car", "cfs", "cpt", "dar", "dd", "dgc", "dmg", "ear", "gca", "genozip", "ha", "hki", "ice", "iso", "jar", "kgb", "lzh", "lha", "lzx", "pak", "partimg", "paq6", "paq7", "paq8", "pea", "phar", "pim", "pit", "qda", "rar", "rk", "sda", "sea", "sen", "sfx", "shk", "sit", "sitx", "sqx", "tar.gz", "tgz", "tar.z", "tar.bz2", "tbz2", "tar.lz", "tlz", "tar.xz", "txz", "tar.zst", "uc", "uc0", "uc2", "ucn", "ur", "ue2", "uca", "uha", "war", "wim", "xar", "xp3", "yz1", "zip", "zipx", "zoo", "zpaq", "zz"];

function isExt(group: string[], filename: string) {
  return group.some(ext => filename.endsWith("." + ext));
}

function FileIcon(props: { filename: string }) {
  return (
    <Switch fallback={<AiOutlineFileText />}>
      <Match when={isExt(SPREADSHEET_EXTS, props.filename)}>
        <AiOutlineFileExcel />
      </Match>
      <Match when={isExt(GIF_EXTS, props.filename)}>
        <AiOutlineFileGif />
      </Match>
      <Match when={isExt(IMAGE_EXTS, props.filename)}>
        <AiOutlineFileImage />
      </Match>
      <Match when={isExt(MARKDOWN_EXTS, props.filename)}>
        <AiOutlineFileMarkdown />
      </Match>
      <Match when={isExt(PDF_EXTS, props.filename)}>
        <AiOutlineFilePdf />
      </Match>
      <Match when={isExt(PRESENTATION_EXTS, props.filename)}>
        <AiOutlineFilePpt />
      </Match>
      <Match when={isExt(DOCUMENT_EXTS, props.filename)}>
        <AiOutlineFileWord />
      </Match>
      <Match when={isExt(ARCHIVE_EXTS, props.filename)}>
        <AiOutlineFileZip />
      </Match>
    </Switch>
  );
}