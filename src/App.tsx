import { createSignal, onMount, batch, createEffect } from "solid-js";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/api/notification";
import { open } from "@tauri-apps/api/dialog";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/tauri";
import Content from "./components/content";
import Header from "./components/header";
import "./index.scss";

export default function App() {
  const [ pendingFiles, setPendingFiles ] = createSignal<PendingFile[]>([]);
  const [ files, setFiles ] = createSignal<File[]>([]);
  const [ selected, setSelected ] = createSignal<number[] | null>(null);
  const [ query, setQuery ] = createSignal<string>("");

  onMount(async () => {
    const files: File[] = await invoke<string>("get_files").then(state => JSON.parse(state));
    setFiles(files.reverse());

    // listen("tauri://file-drop", async ({ payload }) => {
    //   console.log(payload);
    // });

    listen<number>("uploading", async ({ payload }) => {
      setPendingFiles(files => {
        const pending = structuredClone(files);
        pending[0].size = payload;
        return pending;
      });
    });

    listen<void>("uploaded", async () => {
      const file = pendingFiles()[0].name;
      let permissionGranted = await isPermissionGranted();
      if (!permissionGranted) {
        const permission = await requestPermission();
        permissionGranted = permission === "granted";
      }

      if (permissionGranted) {
        sendNotification({ title: "File uploaded", body: `The file ${file} has been uploaded` });
      }

      const files = await invoke<string>("get_files").then(state => JSON.parse(state));
      batch(() => {
        setFiles(files.reverse());
        setPendingFiles(files => {
          const pending = structuredClone(files);
          pending.shift();
          return pending;
        });
      });
    });
  });

  async function addFile() {
    const file = await open({});
    if (!file || Array.isArray(file)) return;

    const name = file.split("/").pop()!;
    setPendingFiles(files => [ { name, size: null }, ...files ]);
    await invoke("add_file", { file });
  }

  async function deleteFile() {
    if (selected() === null) return;
    await invoke("delete_file", { file: files()[selected()![0]].created });
  }

  function select(idx: number | null) {
    if (idx === null) return setSelected(null);
    setSelected(selected()?.[0] === idx ? null : [ idx ]);
  }

  createEffect(() => {
    Array.from(document.getElementsByClassName("selected")).forEach(el => el.classList.remove("selected"));
    if (selected() !== null) {
      const files = document.getElementsByClassName("file");
      selected()!.forEach(idx => files[idx].classList.add("selected"));
    }
  });

  return (
    <div class="app">
      <Header addFile={addFile} query={query} setQuery={setQuery} selected={selected} deleteFile={deleteFile} />
      <Content files={files} pendingFiles={pendingFiles} select={select} query={query} />
    </div>
  )
}

