import { batch, createSignal, onMount } from "solid-js";
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

  onMount(async () => {
    const files = await invoke<IFile[]>("get_files");
    const settings = await invoke<ISettings>("get_settings");
    
    batch(() => {
      setFiles(files);
      setSettings(settings);
    
      // if (!settings.token || !settings.channel || !settings.guild) {
      //   setSettingsOpen(true);
      // }  
    });
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
      <Settings
        open={settingsOpen}
        close={() => setSettingsOpen(false)}
        settings={settings}
        setSettings={setSettings}
      />
    </div>
  )
}