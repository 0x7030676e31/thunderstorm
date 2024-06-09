import { JSX } from "solid-js/jsx-runtime";
import styles from "./boxicon.module.scss";

type Props = {
  children: JSX.Element;
  onClick: () => void;
  size?: number;
  disabled?: boolean;
};

export default function BoxIcon(props: Props) {
  return (
    <div
      class={styles.boxicon}
      onClick={props.onClick}
      style={{ "--size": props.size || 36 + "px" }}
      classList={{ [styles.disabled]: props.disabled }}
    >
      {props.children}
    </div>
  );
}