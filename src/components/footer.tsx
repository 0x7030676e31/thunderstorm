import { FaSolidAngleRight } from "solid-icons/fa"
import BoxIcon from "./boxicon";
import styles from "./footer.module.scss";

export default function Footer() {
  return (
    <div class={styles.footer}>
      <div class={styles.left}>
        <div class={styles.text}>
          Uploading files...
        </div>
        <div class={styles.subtext}>
          10.5 MB / 25 MB
        </div>
        <div class={styles.progress}>
          <div class={styles.bar} style={{ width: "42%" }} />
        </div>
      </div>
      <div class={styles.right}>
        <div class={styles.cancel}>
          Cancel
        </div>
        <BoxIcon onClick={() => {}}>
          <FaSolidAngleRight />
        </BoxIcon>
      </div>
    </div>
  );
}