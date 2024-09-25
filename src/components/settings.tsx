import { Accessor, Match, Setter, Switch, batch, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import { invoke } from "@tauri-apps/api";
import { open } from "@tauri-apps/api/dialog";
import { IoWarningOutline, IoEyeOutline, IoEyeOffOutline, IoClose } from "solid-icons/io";
import { AiOutlineExclamationCircle, AiOutlineFolderOpen } from "solid-icons/ai";
import { FaSolidCheck } from "solid-icons/fa";
import styles from "./settings.module.scss";

type Props = {
  open: boolean;
  close: () => void;
  settings: Accessor<ISettings>;
  setSettings: Setter<ISettings>;
}

export default function Settings(props: Props) {
  const [tab, setTab] = createSignal(0);
  const [diff, setDiff] = createSignal(false);
  const [modal, setModal] = createSignal(false);

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

  async function submit(data: { [key: string]: string | boolean }) {
    if (!modal() && tab() === 0 && (data.channel !== props.settings()!.channel || data.guild !== props.settings()!.guild)) {
      setModal(true);
      return;
    }

    const diff = Object.entries(data).filter(([key, value]) => value !== props.settings()[key as keyof ISettings]);
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
          Security & Integrity
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 2 }} onClick={() => changeTab(2)}>
          Application
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 3 }} onClick={() => changeTab(3)}>
          [ ... ]
        </div>
        <div class={styles.tab} classList={{ [styles.active]: tab() === 4 }} onClick={() => changeTab(4)}>
          [ ... ]
        </div>
      </div>

      <div class={styles.content}>
        <Switch>
          <Match when={tab() === 0}>
            <DiscordTab settings={props.settings} setDiff={setDiff} onSubmit={settings => submit(settings)} />
          </Match>
          <Match when={tab() === 1}>
            <SecurityAndIntegrationTab settings={props.settings} setDiff={setDiff} onSubmit={settings => submit(settings)} />
          </Match>
          <Match when={tab() === 2}>
            <ApplicationTab settings={props.settings} setDiff={setDiff} onSubmit={settings => submit(settings)} />
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
  onSubmit: (data: { [key: string]: string | boolean }) => void;
};

function DiscordTab({ settings, setDiff, onSubmit }: TabProps) {
  const [token, setToken] = createSignal(settings().token);
  const [channel, setChannel] = createSignal(settings().channel);
  const [guild, setGuild] = createSignal(settings().guild);
  const [tokenShown, setTokenShown] = createSignal(false);

  let tokenRef: HTMLInputElement | undefined;
  let channelRef: HTMLInputElement | undefined;
  let guildRef: HTMLInputElement | undefined;

  function blink(ref: HTMLInputElement) {
    ref.classList.remove(styles.blink);
    void ref.offsetWidth;
    ref.classList.add(styles.blink);
  }

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
    let error = false;

    if (token().trim() === "") {
      blink(tokenRef!);
      error = true;
    }

    if (channel().trim() === "" || !/^\d+$/.test(channel())) {
      blink(channelRef!);
      error = true;
    }

    if (guild().trim() === "" || !/^\d+$/.test(guild())) {
      blink(guildRef!);
      error = true;
    }

    if (error) {
      return;
    }

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
      <div class={styles.secretText} ref={tokenRef}>
        <input
          type={tokenShown() ? "text" : "password"}
          placeholder="Discord token"
          class={styles.secretTextInput}
          value={token()}
          onInput={(e) => setToken((e.target as HTMLInputElement).value)}
        />

        <div
          class={styles.eye}
          onClick={() => setTokenShown(!tokenShown())}
        >
          {tokenShown() ? <IoEyeOffOutline /> : <IoEyeOutline />}
        </div>
      </div>

      <p class={styles.note}>Changing the token field may break the application in some cases. Use with caution.</p>

      <div class={styles.inline}>
        <IoWarningOutline />
        <h2>Danger zone</h2>
      </div>

      <p class={styles.sublabel}>Changing these settings will earse all metadata and files associated with the current settings.</p>

      <p class={styles.label}>CHANNEL</p>
      <input
        type="text"
        placeholder="Channel ID"
        class={styles.text}
        value={channel()}
        onInput={(e) => setChannel((e.target as HTMLInputElement).value)}
        ref={channelRef}
      />

      <div class={styles.separator} />

      <p class={styles.label}>GUILD</p>
      <input
        type="text"
        placeholder="Guild ID"
        class={styles.text}
        value={guild()}
        onInput={(e) => setGuild((e.target as HTMLInputElement).value)}
        ref={guildRef}
      />
    </div>
  );
}

function SecurityAndIntegrationTab({ settings, setDiff, onSubmit }: TabProps) {
  const [encryption, setEncryption] = createSignal(settings().do_encrypt);
  const [checksum, setChecksum] = createSignal(settings().do_checksum);

  createEffect(() => {
    setDiff(
      encryption() !== settings().do_encrypt ||
      checksum() !== settings().do_checksum
    );
  });

  const reset = () => {
    setEncryption(settings().do_encrypt);
    setChecksum(settings().do_checksum);
  }

  const submit = () => {
    onSubmit({
      do_encrypt: encryption(),
      do_checksum: checksum(),
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
      <h1>Security & Integration</h1>

      <div class={styles.inline}>
        <AiOutlineExclamationCircle />
        <h2>Important</h2>
      </div>

      <p class={styles.sublabel}>Changing these settings may affect the upload speed and the overall performance of the application.</p>

      <Checkbox
        checked={encryption}
        onToggle={setEncryption}
        label="Enable data encryption"
        note="Encrypt all data before uploading it to the server using a randomly generated key. Please note that enabling this option may significantly impact performance and slow down the upload process. Files that are already encrypted will be automatically decrypted upon download."
      />

      <div class={styles.separator} />

      <Checkbox
        checked={checksum}
        onToggle={setChecksum}
        label="Enable file integrity check"
        note="Calculate the checksum of the file before uploading it to the server and verify it after downloading. While this may slightly affect upload performance, it ensures the integrity of the file and prevents corruption during the download process. This setting is recommended for all users."
      />

    </div>
  );
}

function ApplicationTab({ settings, setDiff, onSubmit }: TabProps) {
  const [download, setDownload] = createSignal(settings().download_location);

  createEffect(() => {
    setDiff(
      download() !== settings().download_location
    );
  });

  const reset = () => {
    setDownload(settings().download_location);
  }

  const submit = () => {
    onSubmit({
      download_location: download(),
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
      <h1>Application</h1>

      <p class={styles.label}>DOWNLOAD PATH</p>
      <div class={styles.pathPicker}>
        <div class={styles.path}>{download()}</div>

        <div class={styles.browse} onClick={async () => {
          const path = await open({
            directory: true,
          });

          if (path) {
            setDownload(path as string);
          }
        }}>
          <AiOutlineFolderOpen />
        </div>
      </div>
    </div>
  );
}

type CheckboxProps = {
  checked: Accessor<boolean>;
  onToggle: Setter<boolean>;
  label: string;
  note: string;
}

function Checkbox({ checked, onToggle, label, note }: CheckboxProps) {
  return (
    <div class={styles.checkbox}>
      <div class={styles.checkboxInline}>
        <p class={styles.checkboxLabel}>{label}</p>
        <div class={styles.checkboxInput} onClick={() => onToggle(!checked())} classList={{ [styles.checked]: checked() }}>
          <div class={styles.checkboxCheck} classList={{ [styles.checked]: checked() }}>
            <FaSolidCheck class={styles.tick} classList={{ [styles.checked]: checked() }} />
            <IoClose class={styles.cross} classList={{ [styles.checked]: !checked() }} />
          </div>
        </div>
      </div>

      <p class={styles.checkboxNote}>{note}</p>
    </div>
  );
}
