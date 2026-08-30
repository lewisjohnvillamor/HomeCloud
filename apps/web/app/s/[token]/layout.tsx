import type { Metadata } from "next";
import type { ReactNode } from "react";
import styles from "./share-layout.module.css";

export const metadata: Metadata = {
  title: "Shared",
  // A share link is a private URL; keeping it out of search indexes is
  // the least a self-hosted server should do for its owner.
  robots: { index: false, follow: false },
};

/**
 * The public view. No session provider, no navigation: a visitor sees
 * exactly what the link covers and nothing that hints at the rest.
 */
export default function ShareLayout({ children }: { children: ReactNode }) {
  return <main className={styles.main}>{children}</main>;
}
