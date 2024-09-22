import { render } from "solid-js/web";
import App from "./App";
import "./index.scss";

(() => {
  if (window.location.hostname !== "tauri.localhost") {
    return;
  }

  const args = [(e: Event) => e.preventDefault(), { capture: true }] as const;
  document.addEventListener("contextmenu", ...args);
  document.addEventListener("selectstart", ...args);
})();

render(() => <App />, document.getElementById("root") as HTMLElement);