import { Accessor, createEffect, createSignal } from "solid-js";
import styles from "./settings.module.scss";

type Props = {
  open: Accessor<boolean>;
  close: () => void;
}

export default function Settings(props: Props) {
  const [ tab, setTab ] = createSignal(0);
  
  createEffect(() => {
    if (props.open()) {
      setTab(0);
    }
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && props.open()) {
      props.close();
    }
  });

  return (
    <div
      class={styles.settings}
      classList={{ [styles.open]: props.open() }}
    >
      <div class={styles.tabs}>
        <div class={styles.label}>
          General
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 0 }} onClick={() => setTab(0)}>
          Discord
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 1 }} onClick={() => setTab(1)}>
          Encryption
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 2 }} onClick={() => setTab(2)}>
          Local storage
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 3 }} onClick={() => setTab(3)}>
          Storage
        </div>
      </div>

      <div class={styles.contnet}>
      </div>

      <div class={styles.closeWrapper}>
        <div class={styles.close}>
          <div class={styles.closeButton} onClick={props.close}>
            +
          </div>
          <p>ESC</p>
        </div>
      </div>
    </div>
  );
}