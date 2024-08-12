import { Match, Show, Switch } from "solid-js";
import { IoWarningOutline } from "solid-icons/io";
import styles from "./error.module.scss";

type Props = {
  isOpen: boolean;
  close: () => void;
  openSettings: () => void;
  error: IError | null;
}

export default function ErrorModal(props: Props) {
  const canOpenSettings = () => props.error?.type === "Unauthorized"
    || props.error?.type === "Forbidden"
    || props.error?.type === "NotFound";
  
  return (
    <div class={styles.container} classList={{ [styles.open]: props.error !== null && props.isOpen }}>
      <div class={styles.modal}>
        <div class={styles.header}>
          <IoWarningOutline />
          <Switch>
            <Match when={props.error?.source === "upload"}>
              <h1>Upload Error</h1>
            </Match>
          </Switch>
        </div>

        <div class={styles.body}>
          <Switch>
            <Match when={props.error?.type === "Reqwest"}>
              <h2> Http request failed </h2>
              <p>
                {props.error?.error}
              </p>
            </Match>
            <Match when={props.error?.type === "Unauthorized"}>
              <h2> Unauthorized </h2>
              <p>
                It seems like the token you provided is invalid. Please make sure you have the correct token in your settings.
              </p>
            </Match>
            <Match when={props.error?.type === "Forbidden"}>
              <h2> Forbidden </h2>
              <p>
                The user lacks permissions to access the specified channel or guild. Ensure your token has the necessary permissions.
              </p>
            </Match>
          </Switch>
        </div>

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

          {/* <BoxIcon onClick={props.close}>
            <AiOutlineClose />
          </BoxIcon>
        </div>
        <div class={styles.body}>
          <div class={styles.message}>
            {props.error?.error}
          </div>
          <div class={styles.source}>
            {props.error?.source}
          </div>
          <div class={styles.actions}>
            <button onClick={props.close}>Close</button>
            <button onClick={props.openSettings}>Open Settings</button>
          </div> */}
      </div>
    </div>
  );
}