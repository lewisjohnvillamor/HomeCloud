"use client";

import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button";
import { ErrorState, PendingState } from "@/components/ui/states";
import { fetchRecoveryStatus, regenerateRecoveryCode } from "@/lib/api/endpoints";
import type { RecoveryStatus } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { formatDate } from "@/lib/format";
import { RecoveryCodeNotice } from "./recovery-code";
import styles from "@/components/people/people-section.module.css";

/**
 * The recovery code for the signed-in account.
 *
 * The status endpoint never returns the code itself, so all this can say
 * is whether one exists and when it was made. Replacing it is one action
 * — the thing someone does after a code on paper has been seen.
 */
export function RecoverySection() {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const [issued, setIssued] = useState<string | null>(null);

  const load = useCallback((signal: AbortSignal) => fetchRecoveryStatus({ signal }), []);
  const { state, reload } = useAsyncData<RecoveryStatus>(load);

  async function onRegenerate() {
    setBusy(true);
    setProblem(null);

    const result = await regenerateRecoveryCode();

    if (result.ok) {
      setIssued(result.data.code);
      await reload();
    } else {
      setProblem(result.problem);
    }

    setBusy(false);
  }

  if (issued) {
    return (
      <RecoveryCodeNotice
        code={issued}
        continueLabel="Done"
        onContinue={() => setIssued(null)}
      />
    );
  }

  return (
    <>
      {state.phase === "loading" ? <PendingState label="Checking your recovery code…" /> : null}
      {state.phase === "failed" ? (
        <ErrorState title="Recovery status is unavailable" description={state.problem.detail} />
      ) : null}

      {state.phase === "ready" ? (
        <p className={styles.meta}>
          {state.data.hasCode
            ? `A recovery code was created ${
                state.data.createdAt ? formatDate(state.data.createdAt) : "for this account"
              }. Only its hash is stored, so it cannot be shown again.`
            : "This account has no recovery code. Without one, a forgotten password cannot be reset."}
        </p>
      ) : null}

      <div className={styles.actions}>
        <Button onClick={() => void onRegenerate()} disabled={busy}>
          {state.phase === "ready" && state.data.hasCode
            ? "Replace recovery code"
            : "Create a recovery code"}
        </Button>
      </div>

      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}
    </>
  );
}
