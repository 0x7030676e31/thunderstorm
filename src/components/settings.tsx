import { Accessor, Match, Setter, Switch, batch, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import { IoWarningOutline } from "solid-icons/io";
import styles from "./settings.module.scss";
import { invoke } from "@tauri-apps/api";

type Props = {
  open: boolean;
  close: () => void;
  settings: Accessor<ISettings>;
  setSettings: Setter<ISettings>;
}

export default function Settings(props: Props) {
  const [ tab, setTab ] = createSignal(0);
  const [ diff, setDiff ] = createSignal(false);
  const [ modal, setModal ] = createSignal(false);

  let settingsRef: HTMLDivElement | undefined;

  createEffect(() => {
    if (props.open) {
      setTab(0);
    }
  });

  function shake() {
    settingsRef?.classList.remove(styles.shake);
    void settingsRef?.offsetWidth;
    settingsRef?.classList.add(styles.shake);
  }

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key !== "Escape" || !props.open) {
      return;
    }

    if (modal()) {
      setModal(false);
      return;
    }

    if (diff()) {
      shake();
      return;
    }

    props.close();
  }

  onMount(() => {
    document.addEventListener("keydown", onKeyDown);
  });

  onCleanup(() => {
    document.removeEventListener("keydown", onKeyDown);
  });

  async function submit(data: { [key: string]: string }) {
    if (!modal() && tab() === 0 && (data.channel !== props.settings()!.channel || data.guild !== props.settings()!.guild)) {
      setModal(true);
      return;
    }

    const diff = Object.entries(data).filter(([ key, value ]) => value !== props.settings()[key as keyof ISettings]);
    invoke("set_settings", { settings: Object.fromEntries(diff) });

    batch(() => {
      setModal(false);
      setDiff(false);
      props.setSettings({
        ...props.settings(),
        ...data as unknown as ISettings
      });
    });
  }

  function changeTab(tab: number) {
    if (!diff()) {
      setTab(tab);
    } else {
      shake();
    }
  }

  return (
    <div
      class={styles.settings}
      classList={{ [styles.open]: props.open }}
      ref={settingsRef}
    >
      <div class={styles.tabs}>
        <div class={styles.label}>
          General
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 0 }} onClick={() => changeTab(0)}>
          Discord
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 1 }} onClick={() => changeTab(1)}>
          Encryption
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 2 }} onClick={() => changeTab(2)}>
          Application
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 3 }} onClick={() => changeTab(3)}>
          [TBD]
        </div>
      </div>

      <div class={styles.content}>
        <Switch>
          <Match when={tab() === 0}>
            <DiscordTab settings={props.settings} setDiff={setDiff} onSubmit={settings => submit(settings)} />
          </Match>
        </Switch>
        <div
          class={styles.unsaved}
          classList={{
            [styles.shown]: diff(),
          }}
        >
          <p>
            Careful — you have unsaved changes!
          </p>
          <button
            class={styles.reset}
            onClick={() => {
              setDiff(false);
              document.dispatchEvent(new Event("reset"));
            }}
          >
            Reset
          </button>
          <button
            class={styles.save}
            onClick={() => document.dispatchEvent(new Event("submit"))}
          >
            Save Changes
          </button>
        </div>
      </div>

      <div class={styles.closeWrapper}>
        <div class={styles.close}>
          <div class={styles.closeButton} onClick={() => {
            if (diff()) {
              shake();
              return;
            }

            props.close();
          }}>
            +
          </div>
          <p>ESC</p>
        </div>
      </div>

      <Portal>
        <div class={styles.modal} classList={{ [styles.open]: modal() }}>
          <div class={styles.modalContent}>
            <div class={styles.header}>
              <IoWarningOutline />
              <h1>Warning</h1>
            </div>
            <p>
              By changing these settings, you will erase everything associated with the current discord settings, including all uploaded files. <br /><br />
              Are you sure you want to continue?
            </p>
            <div class={styles.actions}>
              <button class={styles.cancel} onClick={() => setModal(false)}>
                Cancel
              </button>
              <button class={styles.confirm} onClick={() => document.dispatchEvent(new Event("submit"))}>
                Continue
              </button>
            </div>
          </div>
        </div>
      </Portal>
    </div>
  );
}

type TabProps = {
  settings: Accessor<ISettings>;
  setDiff: (diff: boolean) => void;
  onSubmit: (data: { [key: string]: string }) => void;
};

function DiscordTab({ settings, setDiff, onSubmit }: TabProps) {
  const [ token, setToken ] = createSignal(settings().token);
  const [ channel, setChannel ] = createSignal(settings().channel);
  const [ guild, setGuild ] = createSignal(settings().guild);

  createEffect(() => {    
    setDiff(
      token() !== settings().token ||
      channel() !== settings().channel ||
      guild() !== settings().guild
    );
  });

  const reset = () => {
    setToken(settings().token);
    setChannel(settings().channel);
    setGuild(settings().guild);
  };

  const submit = () => {
    onSubmit({
      token: token(),
      channel: channel(),
      guild: guild(),
    });
  }
  
  onMount(() => {
    document.addEventListener("reset", reset);
    document.addEventListener("submit", submit);
  });

  onCleanup(() => {
    document.removeEventListener("reset", reset);
    document.removeEventListener("submit", submit);
  });

  return (
    <div>
      <h1>Discord</h1>

      <p class={styles.label}>TOKEN</p>
      <input type="text" placeholder="Discord token" class={styles.text} value={token()} onInput={(e) => setToken((e.target as HTMLInputElement).value)} />
      <p class={styles.note}>Changing the token field may break the application in some cases. Use with caution.</p>
    
      <div class={styles.inline}>
        <IoWarningOutline />
        <h2>Danger zone</h2>
      </div>

      <p class={styles.sublabel}>Changing these settings will earse all metadata and files associated with the current settings.</p>

      <p class={styles.label}>CHANNEL</p>
      <input type="text" placeholder="Channel ID" class={styles.text} value={channel()} onInput={(e) => setChannel((e.target as HTMLInputElement).value)} />
      <div class={styles.separator} />

      <p class={styles.label}>GUILD</p>
      <input type="text" placeholder="Guild ID" class={styles.text} value={guild()} onInput={(e) => setGuild((e.target as HTMLInputElement).value)} />

    </div>
  );
}
