import { Accessor, Match, Show, Switch } from "solid-js";
import { IoWarningOutline } from "solid-icons/io";
import styles from "./error.module.scss";

type Props = {
  isOpen: boolean;
  close: () => void;
  openSettings: () => void;
  error: Accessor<IError | null>;
}

export default function ErrorModal(props: Props) {
  const canOpenSettings = () => props.error()?.type === "Unauthorized"
    || props.error()?.type === "Forbidden"
    || props.error()?.type === "NotFound";

  return (
    <div class={styles.container} classList={{ [styles.open]: props.error !== null && props.isOpen }}>
      <div class={styles.modal}>
        <div class={styles.header}>
          <IoWarningOutline />
          <ErrorHeading job={() => props.error()?.job ?? ""} />
        </div>

        <ErrorBody type={() => props.error()?.type ?? ""} message={() => props.error()?.message ?? ""} />

        <div class={styles.actions}>
          <button class={styles.cancel} onClick={() => props.close()}>
            Close
          </button>
          <Show when={canOpenSettings()}>
            <button class={styles.confirm} onClick={() => props.openSettings()}>
              Open Settings
            </button>
          </Show>
        </div>
      </div>
    </div>
  );
}

function ErrorHeading({ job }: { job: Accessor<string> }) {
  return (
    <Switch>
      <Match when={job() === "upload"}>
        <h1>Upload Error</h1>
      </Match>
    </Switch>
  );
}

function ErrorBody({ type, message }: { type: Accessor<string>, message: Accessor<string> }) {
  return (
    <div class={styles.body}>
      <Switch>
        <Match when={type() === "Io"}>
          <h2> I/O Error </h2>
          <p>
            Could not access the file or directory: {message()}
          </p>
        </Match>
        <Match when={type() === "Reqwest"}>
          <h2> Http request failed </h2>
          <p>
            {message()}
          </p>
        </Match>
        <Match when={type() === "Unauthorized"}>
          <h2> Unauthorized </h2>
          <p>
            It seems like the token you provided is invalid. Please make sure you have the correct token in your settings.
          </p>
        </Match>
        <Match when={type() === "Forbidden"}>
          <h2> Forbidden </h2>
          <p>
            The user lacks permissions to access the specified channel or guild. Ensure your token has the necessary permissions.
          </p>
        </Match>
      </Switch>
    </div>
  );
}
