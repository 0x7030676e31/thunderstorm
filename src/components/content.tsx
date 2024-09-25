import {
  AiOutlineFileExcel,
  AiOutlineFileGif,
  AiOutlineFileImage,
  AiOutlineFileMarkdown,
  AiOutlineFilePdf,
  AiOutlineFilePpt,
  AiOutlineFileText,
  AiOutlineFileWord,
  AiOutlineFileZip,
  AiOutlineCloudDownload,
  AiOutlineEdit,
  AiOutlineDelete,
  AiOutlineLayout,
  AiOutlineFolderAdd,
  AiOutlineCopy,
} from "solid-icons/ai";
import { BsFileEarmarkLock2Fill } from "solid-icons/bs";
import { Accessor, For, Match, Setter, Show, Switch, batch, createSignal, onCleanup, onMount } from "solid-js";
import { UnlistenFn, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api";
import { writeText } from "@tauri-apps/api/clipboard";
import { unit, filename } from "../utils";
import { Portal } from "solid-js/web";
import styles from "./content.module.scss";

type Props = {
  setSelected: Setter<number[]>;
  selected: Accessor<number[]>;
  files: Accessor<IFile[]>;
  order: Accessor<number[] | null>;
  download: (id: number) => void;
  remove: () => void;
}

const fmtDate = new Intl.DateTimeFormat(undefined, {
  weekday: "short",
  year: "numeric",
  month: "short",
  day: "numeric",
});

const fmtTime = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "numeric",
  second: "numeric",
});

export default function Content({ setSelected, selected, files, order, download, remove }: Props) {
  const [hovering, setHovering] = createSignal(false);
  const [context, setContext] = createSignal<{ x: number, y: number, id: number }>({ x: 0, y: 0, id: -1 });
  const [contextOpen, setContextOpen] = createSignal(false);

  const filelist = () => order() === null ? structuredClone(files()).reverse() : order()!.map(id => files().find(file => file.id === id)!);

  let unlistenDrop: UnlistenFn | null = null;
  let unlistenDropHover: UnlistenFn | null = null;
  let unlistenDropCancelled: UnlistenFn | null = null;

  let ctxHeight = 0;
  let ctxWidth = 0;
  let ctxRef: HTMLDivElement | undefined;

  setTimeout(() => {
    if (ctxRef) {
      ctxHeight = ctxRef.clientHeight;
      ctxWidth = ctxRef.clientWidth;
    }
  }, 0);

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "F2" && selected().length === 1) {
      const event = new CustomEvent(`focus:${selected()[0]}`);
      document.dispatchEvent(event);
      setSelected([]);
      return;
    } else if (e.key === "Escape") {
      setSelected([]);
    } else if (e.key === "Delete") {
      remove();
    }
  }

  function onFocus() {
    if (contextOpen()) {
      setContextOpen(false);
    }
  }

  onMount(async () => {
    window.addEventListener("keydown", onKeydown);
    document.addEventListener("focusName", onFocus);

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
    window.removeEventListener("keydown", onKeydown);
    document.removeEventListener("focusName", onFocus);
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

      <Portal>
        <div
          class={styles.contextOverlay}
          classList={{ [styles.active]: contextOpen() }}
          onClick={() => contextOpen() === true && setContextOpen(false)}
          onContextMenu={e => {
            e.preventDefault();
            if (contextOpen()) {
              setContextOpen(false);
            }
          }}
        >
          <div
            class={styles.context}
            style={{
              top: `min(${context().y}px, calc(100vh - ${ctxHeight}px - 32px)`,
              left: `min(${context().x}px, calc(100vw - ${ctxWidth}px - 16px)`,
            }}
            onClick={e => e.stopPropagation()}
            onContextMenu={e => e.stopPropagation()}
            ref={ctxRef}
          >
            <div class={styles.title}>{filename(files().find(file => file.id === context().id)?.path || "")}</div>
            <div class={styles.separator} />
            <div class={styles.option} onClick={() => {
              setContextOpen(false);
              const name = filename(files().find(file => file.id === context().id)?.path || "");
              if (name !== "") {
                writeText(name);
              }
            }}>
              <AiOutlineCopy />
              Copy name
            </div>
            <div class={styles.option} onClick={() => {
              setContextOpen(false);
              download(context().id);
            }}>
              <AiOutlineCloudDownload />
              Download
            </div>
            <div class={styles.option} onClick={() => {
              setContextOpen(false);
              download(context().id);
            }}>
              <AiOutlineFolderAdd />
              Download to [todo]
            </div>
            <div class={styles.option} onClick={() => {
              setContextOpen(false);
              remove();
            }}>
              <AiOutlineDelete />
              Delete
            </div>
            <div class={styles.option} onClick={() => {
              batch(() => {
                setContextOpen(false);
                setSelected([]);
              });

              const event = new CustomEvent(`focus:${context().id}`);
              setTimeout(() => document.dispatchEvent(event), 0);
            }}>
              <AiOutlineEdit />
              Rename
            </div>
            <div class={styles.separator} />
            <div class={styles.option} onClick={() => setContextOpen(false)}>
              <AiOutlineLayout />
              Details [todo]
            </div>
          </div>
        </div>
      </Portal >

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
            openContextMenu={(x, y) => {
              batch(() => {
                setContext({ x, y, id: file.id });
                setContextOpen(true);
                setSelected([file.id]);
              });
            }}
          />}
        </For>
      </div>
    </div >
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
  id: number;
  selected: Accessor<boolean>;
  onClick: () => void;
  openContextMenu: (x: number, y: number) => void;
};

function File({ selected, onClick, openContextMenu, id, path, name, size, created_at, encryption_key }: IFile & FileProps) {
  const [fileName, setFileName] = createSignal(name || filename(path));
  const [focused, setFocused] = createSignal(false);

  let inputRef: HTMLInputElement | undefined;
  function focus() {
    inputRef?.focus();
    inputRef?.select();
    setFocused(true);

    const event = new CustomEvent("focusName");
    document.dispatchEvent(event);
  }

  async function onBlur() {
    setFocused(false);

    await invoke("rename_file", { id, name: fileName().trim() });
    if (fileName().trim() === "") {
      setFileName(filename(path));
    }

    const event = new CustomEvent("rename", { detail: { id, name: fileName().trim() } });
    document.dispatchEvent(event);
  }

  function unfocus(e: Event) {
    if (focused() && e.target !== inputRef) {
      inputRef?.blur();
    }
  }

  onMount(() => {
    document.addEventListener(`focus:${id}`, focus);
    document.addEventListener("click", unfocus);
    document.addEventListener("contextmenu", unfocus);
    document.addEventListener("unfocus", unfocus);
  });

  onCleanup(() => {
    document.removeEventListener(`focus:${id}`, focus);
    document.removeEventListener("click", unfocus);
    document.removeEventListener("contextmenu", unfocus);
    document.removeEventListener("unfocus", unfocus);
  });

  return (
    <div
      class={styles.file}
      classList={{ [styles.selected]: selected() }}
      onClick={onClick}
      onContextMenu={e => {
        e.preventDefault();
        openContextMenu(e.clientX, e.clientY);

        const event = new CustomEvent("unfocus");
        document.dispatchEvent(event);
      }}
    >
      <div class={styles.icon}>
        <FileIcon filename={fileName()} />
        {encryption_key !== null && <BsFileEarmarkLock2Fill class={styles.encrypted} />}
      </div>
      <div class={styles.name}>
        <input
          type="text"
          value={fileName()}
          onInput={e => setFileName((e.target as HTMLInputElement).value)}
          ref={inputRef}
          onBlur={onBlur}
          onClick={e => {
            if (!focused()) {
              e.preventDefault();
              onClick();
            } else {
              e.stopPropagation();
            }

            return false;
          }}
          onMouseDown={e => {
            if (!focused()) {
              e.preventDefault();
              onClick();
            } else {
              e.stopPropagation();
            }

            return false;
          }}
          onKeyDown={e => {
            if (e.key === "Enter" || e.key === "Escape") {
              (e.target as HTMLInputElement)?.blur();
              onBlur();
            }
          }}
        />
      </div>
      <div class={styles.size}>
        {unit(size)}
      </div>
      <div class={styles.date}>
        {fmtDate.format(new Date(created_at * 1000))},
      </div>
      <div>
        {fmtTime.format(new Date(created_at * 1000))}
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