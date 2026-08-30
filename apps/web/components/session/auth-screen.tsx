import type { ReactNode } from "react";
import styles from "./auth-screen.module.css";

/**
 * Frame for the two screens shown before anyone is signed in.
 *
 * Deliberately composed and centred, unlike the dense views behind it:
 * a first-run screen is the one place the product introduces itself.
 */
export function AuthScreen({
  promise,
  footnote,
  children,
}: {
  promise: string;
  footnote?: string;
  children: ReactNode;
}) {
  return (
    <div className={styles.screen}>
      <div className={styles.lockup}>
        <p className={styles.name}>HomeCloud</p>
        <p className={styles.promise}>{promise}</p>
      </div>

      <div className={styles.card}>{children}</div>

      {footnote ? <p className={styles.footnote}>{footnote}</p> : null}
    </div>
  );
}
