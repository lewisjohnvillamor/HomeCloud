import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { AppShell } from "@/components/app-shell";
import { AuthGate } from "@/components/session/auth-gate";
import { SessionProvider } from "@/components/session/session-provider";
import "./globals.css";

export const metadata: Metadata = {
  title: {
    default: "HomeCloud",
    template: "%s · HomeCloud",
  },
  description: "Your files and photos, on hardware you own.",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>
        <SessionProvider>
          <AppShell>
            {/* Presentation only: the server enforces access on every request. */}
            <AuthGate>{children}</AuthGate>
          </AppShell>
        </SessionProvider>
      </body>
    </html>
  );
}
