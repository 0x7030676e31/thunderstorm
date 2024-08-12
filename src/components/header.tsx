import {
  AiOutlineFileAdd,
  AiOutlineCloudDownload,
  AiOutlineDelete,
  AiOutlineEdit,
  AiOutlineSetting,
} from "solid-icons/ai";

import { open } from "@tauri-apps/api/dialog";
import { invoke } from "@tauri-apps/api";
import BoxIcon from "./boxicon";
import styles from "./header.module.scss";

type Props = {
  openSettings: () => void;
  query: string;
  setQuery: (query: string) => void;
  selected: number;
  download: () => void;
  delete: () => void;
  rename: () => void;
}

export default function Header(props: Props) {
  async function openUploadDialog() {
    const result = await open({
      multiple: true,
      title: "Select files to upload",
    });

    if (result === null) {
      return;
    }

    const files = !Array.isArray(result) ? [result] : result;
    invoke("upload_files", {
      files,
    });
  }

  return (
    <div class={styles.header}>
      <BoxIcon onClick={openUploadDialog}>
        <AiOutlineFileAdd />
      </BoxIcon>

      <input
        type="text"
        placeholder="Search"
        class={`${styles.search} ${styles.gap}`}
        value={props.query}
        onInput={(e) => props.setQuery((e.target as HTMLInputElement).value)}
      />

      <BoxIcon
        onClick={props.download}
        disabled={props.selected === 0}
      >
        <AiOutlineCloudDownload />
      </BoxIcon>

      <BoxIcon
        onClick={props.delete}
        disabled={props.selected === 0}
      >
        <AiOutlineDelete />
      </BoxIcon>

      <BoxIcon
        onClick={props.rename}
        disabled={props.selected !== 1}
      >
        <AiOutlineEdit />
      </BoxIcon>

      <div class={styles.separator} />

      <BoxIcon onClick={props.openSettings}>
        <AiOutlineSetting />
      </BoxIcon>
    </div>
  );
}