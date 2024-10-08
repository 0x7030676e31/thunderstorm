import { Accessor, batch, createEffect, createSignal, onCleanup, onMount, Setter, Show } from "solid-js";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api";

// 1077851429991096342/1254857306001379450

import Header from './components/header';
import Content from "./components/content";
import Footer from './components/footer';
import Settings from "./components/settings";
import ErrorModal from './components/error';
import DeleteModal from "./components/delete";

export default function App() {
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [settings, setSettings] = createSignal<ISettings | null>(null);
  const [files, setFiles] = createSignal<IFile[]>([]);
  const [selected, setSelected] = createSignal<number[]>([]);
  const [query, setQuery] = createSignal<string>("");
  const [order, setOrder] = createSignal<number[] | null>(null);

  const [ready, setReady] = createSignal(false);

  const [errorOpen, setErrorOpen] = createSignal(false);
  const [error, setError] = createSignal<IError | null>(null);

  const [deleteModalOpen, setDeleteModalOpen] = createSignal(false);

  let unlistenEraseFiles: UnlistenFn | null = null;
  let unlistenFileUploaded: UnlistenFn | null = null;
  let unlistenUploadError: UnlistenFn | null = null;
  let unlistenDownloadError: UnlistenFn | null = null;

  function onRename(e: CustomEvent<{ id: number, name: string }>) {
    const { id, name } = e.detail;
    const file = files().find(file => file.id === id);
    if (!file) {
      return;
    }

    file.name = name || file.path;
    setFiles(files());
  }

  onMount(async () => {
    document.addEventListener("rename", onRename as any);

    unlistenEraseFiles = await listen("erase_files", async () => {
      batch(() => {
        setFiles([]);
        setQuery("");
      });
    });

    unlistenFileUploaded = await listen<IFile>("file_uploaded", async data => {
      setFiles(files => [...files, data.payload]);
      if (query().length === 0) {
        return;
      }

      const results = await invoke<number[]>("query", { query: query() });
      setOrder(results);
    });

    unlistenUploadError = await listen<Omit<IError, "job">>("upload_error", async data => {
      batch(() => {
        setErrorOpen(true);
        setError({
          job: "upload",
          ...data.payload,
        });
      });
    });

    unlistenDownloadError = await listen<Omit<IError, "job">>("download_error", async data => {
      batch(() => {
        setErrorOpen(true);
        setError({
          job: "download",
          ...data.payload,
        });
      });
    });

    const settings = JSON.parse(await invoke<string>("get_settings"));
    const files = JSON.parse(await invoke<string>("get_files"));

    console.log(files);

    batch(() => {
      setSettings(settings);
      setFiles(files);
      setReady(true);
    });
  });

  onCleanup(() => {
    document.removeEventListener("rename", onRename as any);
    unlistenEraseFiles?.();
    unlistenFileUploaded?.();
    unlistenUploadError?.();
    unlistenDownloadError?.();
  });

  createEffect(async () => {
    if (query().length === 0) {
      setOrder(null);
      return;
    }

    const results = await invoke<number[]>("query", { query: query() });
    setOrder(results);
  });

  async function deleteSelected() {
    await invoke("delete_files", { files: selected() });
    batch(() => {
      setDeleteModalOpen(false);
      setFiles(files => files.filter(file => !selected().includes(file.id)));
      setSelected([]);
    });
  }

  return (
    <div class="app">
      <Header
        openSettings={() => setSettingsOpen(true)}
        query={query()}
        setQuery={setQuery}
        selected={selected().length}
        download={() => invoke("download_files", { files: selected() })}
        delete={() => setDeleteModalOpen(true)}
        rename={() => {
          if (selected().length !== 1) {
            return;
          }

          const event = new CustomEvent(`focus:${selected()[0]}`);
          setTimeout(() => document.dispatchEvent(event), 0);
          setSelected([]);
        }}
      />

      <Show when={ready()}>
        <Content
          setSelected={setSelected}
          selected={selected}
          files={files}
          order={order}
          download={async file => await invoke("download_files", { files: [file] })}
          remove={() => setDeleteModalOpen(true)}
        />
      </Show>

      <Footer />

      <Show when={settings() !== null}>
        <Settings
          open={settingsOpen()}
          close={() => setSettingsOpen(false)}
          settings={settings as Accessor<ISettings>}
          setSettings={setSettings as Setter<ISettings>}
        />
      </Show>

      <ErrorModal
        isOpen={errorOpen()}
        close={() => setErrorOpen(false)}
        openSettings={() => {
          batch(() => {
            setSettingsOpen(true)
            setErrorOpen(false);
          });
        }}
        error={error}
      />

      <DeleteModal
        isOpen={deleteModalOpen}
        confirm={deleteSelected}
        cancel={() => setDeleteModalOpen(false)}
      />
    </div>
  );
}

// export default function App() {
//   const [settingsOpen, setSettingsOpen] = createSignal(false);
//   const [settings, setSettings] = createSignal<ISettings | null>(null);
//   const [files, setFiles] = createSignal<IFile[]>([]);
//   const [selected, setSelected] = createSignal<number[]>([]);
//   const [query, setQuery] = createSignal<string>("");
//   const [error, setError] = createSignal<IError | null>(null);
//   const [errorOpen, setErrorOpen] = createSignal(false);

//   // const [ error, setError ] = createSignal<IError | null>({
//   //   source: "upload",
//   //   type: "Unauthorized",
//   //   error: "It seems like the token you provided is invalid. Please make sure you have the correct token in your settings.",
//   // });

//   // const [ errorOpen, setErrorOpen ] = createSignal(true);

//   let unlistenErase: UnlistenFn | null = null;
//   let unlistenUploaded: UnlistenFn | null = null;
//   let unlistenUploadError: UnlistenFn | null = null;

//   onMount(async () => {
//     unlistenErase = await listen("erase", async () => {
//       batch(() => {
//         setFiles([]);
//         setSelected([]);
//         setQuery("");
//       });
//     });

//     unlistenUploaded = await listen<IFile>("uploaded", async data => {
//       setFiles(files => [...files, data.payload]);
//     });

//     unlistenUploadError = await listen<Omit<IError, "source">>("upload_error", async data => {
//       batch(() => {
//         setErrorOpen(true);
//         setError({
//           source: "upload",
//           ...data.payload,
//         });
//       });
//     });

//     const files = JSON.parse(await invoke<string>("get_files"));
//     const settings = JSON.parse(await invoke<string>("get_settings"));

//     batch(() => {
//       setFiles(files);
//       setSettings(settings);
//     });
//   });

//   onCleanup(() => {
//     unlistenErase?.();
//     unlistenUploaded?.();
//     unlistenUploadError?.();
//   });

//   function downloadSelected() { }

//   function deleteSelected() { }

//   function renameSelected() { }

//   return (
//     <div class="app">
//       <Header
//         openSettings={() => setSettingsOpen(true)}
//         query={query()}
//         setQuery={query => setQuery(query)}
//         selected={selected()}
//         download={downloadSelected}
//         delete={deleteSelected}
//         rename={renameSelected}
//       />
//       <Content
//         setSelected={setSelected}
//         selected={selected()}
//         files={files()}
//       />
//       <Footer />
//       <Show when={settings() !== null}>
//         <Settings
//           open={settingsOpen()}
//           close={() => setSettingsOpen(false)}
//           settings={settings as Accessor<ISettings>}
//           setSettings={setSettings as Setter<ISettings>}
//         />
//       </Show>
//       <ErrorModal
//         isOpen={errorOpen()}
//         close={() => setErrorOpen(false)}
//         openSettings={() => {
//           batch(() => {
//             setSettingsOpen(true)
//             setErrorOpen(false);
//           });
//         }}
//         error={error()}
//       />
//     </div>
//   )
// }