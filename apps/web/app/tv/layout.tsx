import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { AuthGate } from "@/components/session/auth-gate";
import { SessionProvider } from "@/components/session/session-provider";
import styles from "./tv-layout.module.css";

export const metadata: Metadata = {
  title: "TV",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  // A television is a fixed, distant display; pinch-zoom is not the
  // interaction model and the layout is sized for the room instead.
  maximumScale: 1,
  themeColor: "#000000",
};

/**
 * The living-room layout: dark, borderless, and without the application
 * shell. A remote has four directions and two buttons, so the TV gets
 * its own interaction model rather than a stretched desktop UI.
 */
export default function TvLayout({ children }: { children: ReactNode }) {
  return (
    <div className={styles.tv}>
      <SessionProvider>
        <main className={styles.main}>
          <AuthGate>{children}</AuthGate>
        </main>
      </SessionProvider>
    </div>
  );
}
