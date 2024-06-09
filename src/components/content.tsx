import { Accessor, Setter } from "solid-js";
import styles from "./content.module.scss";

type Props = {
  setSelected: Setter<number[]>;
  selected: Accessor<number[]>;
  files: Accessor<IFile[]>;
}


export default function Content(props: Props) {
  return (
    <div class={styles.content}>

    </div>
  );
}