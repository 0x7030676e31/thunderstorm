import { createSignal, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api";

import Header from "./components/header";
import Content from "./components/content";
import Footer from "./components/footer";
import Settings from "./components/settings";

export default function App() {
  const [ settings, setSettings ] = createSignal(false);
  const [ files, setFiles ] = createSignal<IFile[]>([]);
  const [ selected, setSelected ] = createSignal<number[]>([]);
  const [ query, setQuery ] = createSignal<string>("");

  onMount(async () => {
    const files = await invoke<IFile[]>("get_files");
    files.push({
      id: 0,
      name: "example.txt",
      size: 1024,
      created: Date.now(),
    });

    files.push({
      id: 1,
      name: "example2.txt",
      size: 2048,
      created: Date.now(),
    });

    setFiles(files);
  });

  function downloadSelected() {}
  
  function deleteSelected() {}

  function renameSelected() {}

  return (
    <div class="app">
      <Header
        openSettings={() => setSettings(true)}
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
        open={settings}
        close={() => setSettings(false)}
      />
    </div>
  )
}