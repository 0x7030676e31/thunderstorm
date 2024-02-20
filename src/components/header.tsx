import { AiOutlineCloudDownload, AiOutlineDelete, AiOutlineEdit } from 'solid-icons/ai'
import { FiPlus } from "solid-icons/fi";
import { Accessor } from "solid-js";
import styles from "../styles/header.module.scss";

type Props = {
  addFile: () => void,
  query: Accessor<string>,
  setQuery: (query: string) => void,
  selected: Accessor<number[] | null>,
  deleteFile: () => void,
}

export default function Header({ addFile, query, setQuery, selected, deleteFile }: Props) {
  return (
    <div class={styles.header}>
      <div class={styles.left}>
        <div class={styles.box_icon} onClick={addFile}>
          <FiPlus />
        </div>
        <div class={styles.search}>
          <input type="text" placeholder="Search..." value={query()} onInput={e => setQuery(e.currentTarget.value)} />
        </div>
      </div>
      <div class={styles.right}>
        <div class={`${styles.box_icon} ${selected() === null && styles.disabled}`}>
          <AiOutlineCloudDownload />
        </div>
        <div class={`${styles.box_icon} ${selected() === null && styles.disabled}`}>
          <AiOutlineEdit />
        </div>
        <div class={`${styles.box_icon} ${selected() === null && styles.disabled}`} onClick={deleteFile}>
          <AiOutlineDelete />
        </div>
      </div>
    </div>
  )
}