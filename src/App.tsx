import { Accessor, batch, createSignal, onCleanup, onMount, Setter, Show } from "solid-js";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api";

import Header from "./components/header";
import Content from "./components/content";
import Footer from "./components/footer";
import Settings from "./components/settings";

export default function App() {
  const [ settingsOpen, setSettingsOpen ] = createSignal(false);
  const [ settings, setSettings ] = createSignal<ISettings | null>(null);
  const [ files, setFiles ] = createSignal<IFile[]>([]);
  const [ selected, setSelected ] = createSignal<number[]>([]);
  const [ query, setQuery ] = createSignal<string>("");

  let unlistenEraseData: UnlistenFn | null = null;

  onMount(async () => {
    unlistenEraseData = await listen("erase", async () => {
      console.log("Data erased");
    });
    
    const files = JSON.parse(await invoke<string>("get_files"));
    const settings = JSON.parse(await invoke<string>("get_settings"));
    
    batch(() => {
      setFiles(files);
      setSettings(settings);
    });
  });

  onCleanup(() => {
    unlistenEraseData?.();
  });

  function downloadSelected() {}
  
  function deleteSelected() {}

  function renameSelected() {}

  return (
    <div class="app">
      <Header
        openSettings={() => setSettingsOpen(true)}
        query={query}
        setQuery={setQuery}
        selected={selected}
        download={downloadSelected}
        delete={deleteSelected}
        rename={renameSelected}
      />
      <Content
        setSelected={setSelected}
        selected={selected}
        files={files}
      />
      <Footer />
      <Show when={settings() !== null}>
        <Settings
          open={settingsOpen}
          close={() => setSettingsOpen(false)}
          settings={settings as Accessor<ISettings>}
          setSettings={setSettings as Setter<ISettings>}
        />
      </Show>
    </div>
  )
}