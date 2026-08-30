"use client";

import { use, useCallback, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import { fetchPublicUploadRequest, sendToUploadRequest } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { PublicUploadRequest } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { formatBytes } from "@/lib/format";
import styles from "./send.module.css";

/**
 * Sending files to someone else's folder.
 *
 * The mirror image of a share page: there is nothing to read here. The
 * sender is told what the folder is called and how much it will still
 * accept, and nothing at all about what is already in it — which is the
 * point of the feature.
 */
export default function SendPage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = use(params);
  const input = useRef<HTMLInputElement>(null);

  const [sent, setSent] = useState<string[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchPublicUploadRequest(token, { signal }),
    [token],
  );
  const { state, reload } = useAsyncData<PublicUploadRequest>(load);

  async function onSend(files: FileList | null) {
    if (!files || files.length === 0) {
      return;
    }

    setProblem(null);

    for (const file of Array.from(files)) {
      setBusy(file.name);
      const result = await sendToUploadRequest(token, file.name, file);

      if (result.ok) {
        setSent((current) => [...current, result.data.name]);
      } else {
        setProblem(result.problem);
        break;
      }
    }

    setBusy(null);
    await reload();

    if (input.current) {
      input.current.value = "";
    }
  }

  if (state.phase === "loading") {
    return <PendingState label="Opening the link…" />;
  }

  if (state.phase === "failed") {
    // Unknown, expired, and revoked look the same on purpose.
    return state.problem.code === "not_found" ? (
      <EmptyState
        title="This link is not available"
        description="It may have expired, been revoked, or never existed. Ask whoever sent it for a new one."
      />
    ) : (
      <ErrorState
        title="The link could not be opened"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  const request = state.data;
  const full = request.filesLeft <= 0 || request.bytesLeft <= 0;

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <p className={styles.brand}>Shared from HomeCloud</p>
        <h1 className={styles.title}>{request.title}</h1>
        <p className={styles.meta}>
          Files you choose go straight into <strong>{request.folderName}</strong>. You cannot
          see what is already there, and nothing else in the library is reachable from this
          page.
        </p>
      </header>

      {full ? (
        <EmptyState
          title="This link is full"
          description="It has received everything it was set up to accept. Ask whoever sent it for a new one."
        />
      ) : (
        <div className={styles.panel}>
          <p className={styles.allowance}>
            Room for {request.filesLeft} more {request.filesLeft === 1 ? "file" : "files"}, up
            to {formatBytes(request.bytesLeft)}.
          </p>

          <label className={styles.choose}>
            <input
              ref={input}
              className={styles.hidden}
              type="file"
              multiple
              aria-label="Choose files to send"
              disabled={busy !== null}
              onChange={(event) => void onSend(event.target.files)}
            />
            <Button
              variant="primary"
              disabled={busy !== null}
              onClick={() => input.current?.click()}
            >
              {busy ? `Sending ${busy}…` : "Choose files to send"}
            </Button>
          </label>
        </div>
      )}

      {problem ? (
        <ErrorState title="That did not send" description={problem.detail} />
      ) : null}

      {sent.length > 0 ? (
        <section className={styles.sent} aria-label="Files sent">
          <h2 className={styles.sentTitle}>Sent</h2>
          <ul className={styles.list}>
            {sent.map((name) => (
              <li key={name} className={styles.row}>
                <svg
                  className={styles.tick}
                  width="1em"
                  height="1em"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden="true"
                >
                  <path d="m4 12.5 5 5L20 6.5" />
                </svg>
                {name}
              </li>
            ))}
          </ul>
          <p className={styles.meta}>
            These are with {request.folderName} now. You cannot take them back from here —
            ask whoever sent you the link.
          </p>
        </section>
      ) : null}
    </div>
  );
}
