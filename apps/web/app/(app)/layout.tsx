import type { ReactNode } from "react";
import { AppShell } from "@/components/app-shell";
import { AuthGate } from "@/components/session/auth-gate";
import { SessionProvider } from "@/components/session/session-provider";

/** The signed-in product: navigation, session, and the pages behind it. */
export default function AppLayout({ children }: { children: ReactNode }) {
  return (
    <SessionProvider>
      <AppShell>
        {/* Presentation only: the server enforces access on every request. */}
        <AuthGate>{children}</AuthGate>
      </AppShell>
    </SessionProvider>
  );
}
