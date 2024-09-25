import { render } from "solid-js/web";
import App from "./App";
import "./index.scss";

(() => {
  const args = [(e: Event) => e.preventDefault(), { capture: true }] as const;
  document.addEventListener("contextmenu", ...args);
  document.addEventListener("selectstart", ...args);
})();

render(() => <App />, document.getElementById("root") as HTMLElement);