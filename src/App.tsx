import { createSignal, onMount, For, Show } from "solid-js";
import { open } from "@tauri-apps/api/dialog";
import { invoke } from "@tauri-apps/api";
import { Portal } from "solid-js/web";
import { FiPlus } from "solid-icons/fi";
import { AiFillFileText } from 'solid-icons/ai'
import "./index.scss";

interface File {
  name: string,
  size: number,
  clusters: string[],
  modified: number,
  created: number,
}

interface State {
  token: string | null,
  aes_key: string | null,
  storage_channel: string | null,
  files: File[],
}

export default function App() {
  const [ files, setFiles ] = createSignal<File[]>([]);
  const [ modal, setModal ] = createSignal<boolean>(false);

  let token!: HTMLInputElement;
  let aes_key!: HTMLInputElement;
  let storage_channel!: HTMLInputElement;

  async function submit() {
    setModal(false);
    await invoke("set_state", {
      token: token.value,
      aesKey: aes_key.value,
      storageChannel: storage_channel.value,
    });
  }

  async function addFile() {
    const file = await open({});
    if (!file) return;
  
    const data = await invoke<string>("add_file", { file }).then<File>(JSON.parse);
    setFiles([...files(), data]);
  }

  onMount(async () => {
    const state = await invoke<string>("get_state").then<State>(JSON.parse);
    setFiles(state.files);

    token.value = state.token || "";
    aes_key.value = state.aes_key || "";
    storage_channel.value = state.storage_channel || "";

    if (!state.token || !state.aes_key || !state.storage_channel) {
      setModal(true);
    }
  });

  return (
    <div class="content">
      <Show when={modal()}>
        <Portal mount={document.body}>
          <div class="modal">
            <p>Token</p>
            <input type="text" ref={token} />
            <p>AES Key</p>
            <input type="text" ref={aes_key} />
            <p>Storage Channel</p>
            <input type="text" ref={storage_channel} />
            <br/>
            <button onClick={submit}>Submit</button>
          </div>
        </Portal>
      </Show>

      <Header addFile={addFile} />
      <div class="files">
        <For each={files()}>
          {(file, idx) => <File file={file} idx={idx()} />}
        </For>
      </div>
    </div>
  )
}

function Header({ addFile }: { addFile: () => void }) {
  return <div class="header">
    <div class="left">
      <div class="box-icon" onClick={addFile}>
        <FiPlus />
      </div>
    </div>
    <div class="right"></div>
  </div>
}

function unit(bytes: number) {
  const units = ["Bytes", "KiB", "MiB", "GiB", "TiB"];
  let i = 0;
  while (bytes >= 1024) {
    bytes /= 1024;
    i++;
  }
  return `${+bytes.toFixed(2)} ${units[i]}`;
}

function File({ file, idx }: { file: File, idx: number }) {
  return <div class={`file ${idx % 2 === 0 ? "even" : "odd"}`}>
    <AiFillFileText />
    <div class="info">
      <p>{file.name}</p>
      <p>{unit(file.size)}</p>
    </div>
  </div>
}
