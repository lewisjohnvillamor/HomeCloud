"use client";

import type { ReactNode } from "react";
import { ErrorState, PendingState } from "@/components/ui/states";
import { isRetryable } from "@/lib/api/problem";
import { SetupForm, SignInForm } from "./auth-forms";
import { useSession } from "./session-provider";

/**
 * Decides what a page can render: the setup screen on a fresh
 * deployment, the sign-in screen when nobody is signed in, an actionable
 * error when the server cannot be reached, and otherwise the page.
 *
 * This is presentation only. The server enforces access on every
 * request, so bypassing this in a browser reveals nothing.
 */
export function AuthGate({ children }: { children: ReactNode }) {
  const { state, refresh } = useSession();

  if (state.phase === "loading") {
    return <PendingState label="Checking the server…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="The server is not responding"
        description={
          state.problem.requestId
            ? `${state.problem.detail} Reference ${state.problem.requestId}.`
            : state.problem.detail
        }
        actionLabel={isRetryable(state.problem) ? "Try again" : undefined}
        onAction={isRetryable(state.problem) ? () => void refresh() : undefined}
      />
    );
  }

  if (state.phase === "anonymous") {
    return state.needsOwner ? <SetupForm /> : <SignInForm />;
  }

  return <>{children}</>;
}
