"use client";

import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button";
import { ErrorState, PendingState } from "@/components/ui/states";
import { fetchUploadRequests, revokeUploadRequest } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { UploadRequest } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { formatBytes, formatDate } from "@/lib/format";
import styles from "@/components/people/people-section.module.css";

/**
 * Live upload links for a library.
 *
 * These are the only links in the product that let a stranger write, so
 * they are listed with what each has already received and can be
 * switched off in one action.
 */
export function RequestSection({ library }: { library: string }) {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchUploadRequests(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<UploadRequest[]>(load);

  async function onRevoke(link: UploadRequest) {
    setBusy(true);
    setProblem(null);

    const result = await revokeUploadRequest(link.id);
    if (!result.ok) {
      setProblem(result.problem);
    }

    await reload();
    setBusy(false);
  }

  return (
    <>
      {state.phase === "loading" ? <PendingState label="Loading upload links…" /> : null}
      {state.phase === "failed" ? (
        <ErrorState title="Upload links could not be loaded" description={state.problem.detail} />
      ) : null}

      {state.phase === "ready" && state.data.length === 0 ? (
        <p className={styles.meta}>
          No upload links. Make one from a folder in Files — choose “Ask for files” — when you
          want someone to send you something without giving them an account.
        </p>
      ) : null}

      {state.phase === "ready" && state.data.length > 0 ? (
        <ul className={styles.list}>
          {state.data.map((link) => (
            <li key={link.id} className={styles.row}>
              <span>
                <span className={styles.name}>{link.title}</span>
                <span className={styles.meta}>
                  {" "}
                  into {link.folderName} · {link.receivedFiles} of {link.maxFiles} files ·{" "}
                  {formatBytes(link.receivedBytes)} received ·{" "}
                  {link.expiresAt ? `expires ${formatDate(link.expiresAt)}` : "no expiry"}
                </span>
              </span>
              <Button variant="quiet" onClick={() => void onRevoke(link)} disabled={busy}>
                Revoke<span className={styles.hidden}> {link.title}</span>
              </Button>
            </li>
          ))}
        </ul>
      ) : null}

      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}
    </>
  );
}
