import { Accessor } from "solid-js";
import styles from "./delete.module.scss";

type Props = {
  isOpen: Accessor<boolean>;
  confirm: () => void;
  cancel: () => void;
}

export default function DeleteModal({ isOpen, confirm, cancel }: Props) {
  return (
    <div class={styles.container} classList={{ [styles.open]: isOpen() }}>
      <div class={styles.modal}>
        <div class={styles.header}>
          <h1>Delete Files</h1>
        </div>

        <div class={styles.body}>
          <p>Are you sure you want to delete the selected files?</p>
        </div>

        <div class={styles.actions}>
          <button class={styles.cancel} onClick={cancel}>
            Cancel
          </button>
          <button class={styles.confirm} onClick={confirm}>
            Delete
          </button>
        </div>
      </div>
    </div>
  );
}