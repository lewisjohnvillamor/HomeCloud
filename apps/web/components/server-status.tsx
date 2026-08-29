"use client";

import { useCallback, useEffect, useState } from "react";
import { fetchBootstrapStatus, type BootstrapStatus } from "@/lib/api/bootstrap";
import type { ApiProblem } from "@/lib/api/problem";
import { isRetryable } from "@/lib/api/problem";
import { EmptyState, ErrorState, PendingState } from "./ui/states";

type State =
  | { phase: "pending" }
  | { phase: "ready"; status: BootstrapStatus }
  | { phase: "failed"; problem: ApiProblem };

/**
 * Reports whether the server is reachable and set up. This is the only
 * thing the app can honestly say before a library exists, so it is what
 * the home screen shows.
 */
export function ServerStatus() {
  const [state, setState] = useState<State>({ phase: "pending" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    let active = true;

    fetchBootstrapStatus({ signal: controller.signal }).then((result) => {
      if (!active) {
        return;
      }

      setState(
        result.ok
          ? { phase: "ready", status: result.data }
          : { phase: "failed", problem: result.problem },
      );
    });

    return () => {
      active = false;
      controller.abort();
    };
  }, [attempt]);

  // Resetting to pending here rather than inside the effect keeps the
  // effect to a single state transition per attempt.
  const retry = useCallback(() => {
    setState({ phase: "pending" });
    setAttempt((value) => value + 1);
  }, []);

  if (state.phase === "pending") {
    return <PendingState label="Checking the server…" />;
  }

  if (state.phase === "failed") {
    const { problem } = state;

    return (
      <ErrorState
        title="The server is not responding"
        description={
          problem.requestId
            ? `${problem.detail} Reference ${problem.requestId}.`
            : problem.detail
        }
        actionLabel={isRetryable(problem) ? "Try again" : undefined}
        onAction={isRetryable(problem) ? retry : undefined}
      />
    );
  }

  if (state.status.needsOwner) {
    return (
      <EmptyState
        title="This deployment is not set up yet"
        description="No owner account exists. Creating the first account is the next step in setup."
      />
    );
  }

  return (
    <EmptyState
      title="Your library is empty"
      description="The server is set up. Files and photos appear here once a library folder is scanned."
    />
  );
}
