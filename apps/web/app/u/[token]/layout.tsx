import type { Metadata } from "next";
import type { ReactNode } from "react";
import styles from "./send-layout.module.css";

export const metadata: Metadata = {
  title: "Send files",
  // An upload link is a private URL, and one that grants writing at
  // that: keeping it out of search indexes is the least a self-hosted
  // server should do for its owner.
  robots: { index: false, follow: false },
};

/**
 * The public send view. No session provider, no navigation: whoever
 * holds the link sees a folder's name and a file picker, and nothing
 * that hints at the rest of the library.
 */
export default function SendLayout({ children }: { children: ReactNode }) {
  return <main className={styles.main}>{children}</main>;
}
