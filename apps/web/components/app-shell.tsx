import type { ReactNode } from "react";
import { PrimaryNav } from "./navigation/primary-nav";
import styles from "./app-shell.module.css";

/**
 * The frame every page renders inside: a skip link, the primary
 * navigation, and one landmark `main` region.
 */
export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className={styles.shell}>
      <a className={styles.skipLink} href="#main-content">
        Skip to content
      </a>
      <PrimaryNav />
      <main id="main-content" className={styles.main} tabIndex={-1}>
        <div className={styles.content}>{children}</div>
      </main>
    </div>
  );
}
