import type { Metadata } from "next";
import type { ReactNode } from "react";
import styles from "./invite-layout.module.css";

export const metadata: Metadata = {
  title: "Invitation",
  // An invitation link is a private URL; it does not belong in an index.
  robots: { index: false, follow: false },
};

/** No session provider and no navigation: the visitor has no account yet. */
export default function InviteLayout({ children }: { children: ReactNode }) {
  return <main className={styles.main}>{children}</main>;
}
