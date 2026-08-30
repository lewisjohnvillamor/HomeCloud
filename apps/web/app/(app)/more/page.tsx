"use client";

import { useCallback, useEffect, useState } from "react";
import type { ApiResult } from "@/lib/api/client";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { Button } from "@/components/ui/button";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import { useActiveLibrary, useSession } from "@/components/session/session-provider";
import {
  fetchScanStatus,
  fetchSharesForLibrary,
  fetchTrash,
  restoreItem,
  revokeShare,
  startScan,
} from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { Item, ScanStatus, Share } from "@/lib/api/types";
import { formatBytes } from "@/lib/format";
import styles from "./more.module.css";

/** How often a running scan is re-checked. Slow enough to stay quiet. */
const SCAN_POLL_MS = 1500;

/** What the More page needs in one look at the server. */
type Overview = { scan: ScanStatus | null; trash: Item[]; shares: Share[] };

export default function MorePage() {
  const { state, signOut } = useSession();
  const library = useActiveLibrary();

  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const libraryId = library?.id ?? null;

  const load = useCallback(
    async (signal: AbortSignal): Promise<ApiResult<Overview>> => {
      if (!libraryId) {
        return { ok: true, data: { scan: null, trash: [], shares: [] } };
      }

      const [scan, trash, shares] = await Promise.all([
        fetchScanStatus(libraryId, { signal }),
        fetchTrash(libraryId, { signal }),
        fetchSharesForLibrary(libraryId, { signal }),
      ]);

      if (!scan.ok) {
        return scan;
      }
      if (!trash.ok) {
        return trash;
      }
      if (!shares.ok) {
        return shares;
      }

      return {
        ok: true,
        data: { scan: scan.data, trash: trash.data, shares: shares.data },
      };
    },
    [libraryId],
  );

  const { state: overview, reload } = useAsyncData<Overview>(load);
  const scan = overview.phase === "ready" ? overview.data.scan : null;
  const trash = overview.phase === "ready" ? overview.data.trash : null;
  const shares = overview.phase === "ready" ? overview.data.shares : null;

  // Poll only while a scan is actually running, then stop. Nothing here
  // sets state directly; the reload does it from its own callback.
  useEffect(() => {
    if (!scan?.running) {
      return;
    }

    const timer = setInterval(() => void reload(), SCAN_POLL_MS);

    return () => clearInterval(timer);
  }, [scan?.running, reload]);

  async function onScan() {
    if (!libraryId) {
      return;
    }

    setProblem(null);
    setNotice(null);
    const result = await startScan(libraryId);

    if (result.ok) {
      setNotice("Scan started. It runs in the background.");
      await reload();
    } else {
      setProblem(result.problem);
    }
  }

  async function onRevokeShare(share: Share) {
    const result = await revokeShare(share.id);

    if (result.ok) {
      setNotice(`The link to “${share.itemName}” was revoked.`);
      await reload();
    } else {
      setProblem(result.problem);
    }
  }

  async function onRestore(item: Item) {
    const result = await restoreItem(item.id);

    if (result.ok) {
      setNotice(`“${item.name}” restored.`);
      await reload();
    } else {
      setProblem(result.problem);
    }
  }

  return (
    <>
      <h1>More</h1>

      <section className={styles.section} aria-labelledby="account-heading">
        <h2 id="account-heading" className={styles.heading}>
          Account
        </h2>
        <p className={styles.detail}>
          Signed in as{" "}
          <strong>
            {state.phase === "signed-in" ? (state.session.displayName ?? "this account") : "…"}
          </strong>
          .
        </p>
        <Button onClick={() => void signOut()}>Sign out</Button>
      </section>

      <section className={styles.section} aria-labelledby="library-heading">
        <h2 id="library-heading" className={styles.heading}>
          Library
        </h2>
        {library ? (
          <>
            <p className={styles.detail}>
              <strong>{library.name}</strong>
              {library.rootPath ? (
                <>
                  {" — files live in "}
                  <code className={styles.code}>{library.rootPath}</code> on the server.
                </>
              ) : null}
            </p>
            <p className={styles.detail}>
              A scan reads that folder and updates the catalog. It never moves or deletes
              your files.
            </p>
            <div className={styles.actions}>
              <Button variant="primary" onClick={() => void onScan()} disabled={scan?.running}>
                {scan?.running ? "Scanning…" : "Scan library"}
              </Button>
            </div>
            {scan?.running ? <PendingState label="Scanning the library…" /> : null}
            {scan && !scan.running && scan.scanned !== null ? (
              <p className={styles.detail} role="status">
                Last scan indexed {scan.scanned} item{scan.scanned === 1 ? "" : "s"}
                {scan.missing ? `, and marked ${scan.missing} as missing` : ""}.
              </p>
            ) : null}
            {scan?.error ? (
              <ErrorState title="The last scan failed" description={scan.error} />
            ) : null}
          </>
        ) : (
          <EmptyState
            title="No library yet"
            description="This account is not a member of any library."
          />
        )}
      </section>

      <section className={styles.section} aria-labelledby="shares-heading">
        <h2 id="shares-heading" className={styles.heading}>
          Shared links
        </h2>
        <p className={styles.detail}>
          Anyone holding one of these links can read that item without signing
          in. Revoking a link takes effect immediately.
        </p>
        {shares === null ? <PendingState label="Loading links…" /> : null}
        {shares?.length === 0 ? (
          <p className={styles.detail}>Nothing in this library is shared.</p>
        ) : null}
        {shares && shares.length > 0 ? (
          <ul className={styles.trashList}>
            {shares.map((share) => (
              <li key={share.id} className={styles.trashItem}>
                <span>
                  <span className={styles.trashName}>{share.itemName}</span>
                  <span className={styles.detail}>
                    {" "}
                    {share.expiresAt ? "expires soon" : "no expiry"} · opened{" "}
                    {share.accessCount} time{share.accessCount === 1 ? "" : "s"}
                  </span>
                </span>
                <Button variant="quiet" onClick={() => void onRevokeShare(share)}>
                  Revoke<span className={styles.hidden}> the link to {share.itemName}</span>
                </Button>
              </li>
            ))}
          </ul>
        ) : null}
      </section>

      <section className={styles.section} aria-labelledby="trash-heading">
        <h2 id="trash-heading" className={styles.heading}>
          Trash
        </h2>
        <p className={styles.detail}>
          Deleted items are moved into a folder inside your library and stay on disk until
          you remove them yourself.
        </p>
        {trash === null ? <PendingState label="Loading the trash…" /> : null}
        {trash?.length === 0 ? (
          <p className={styles.detail}>The trash is empty.</p>
        ) : null}
        {trash && trash.length > 0 ? (
          <ul className={styles.trashList}>
            {trash.map((item) => (
              <li key={item.id} className={styles.trashItem}>
                <span>
                  <span className={styles.trashName}>{item.name}</span>
                  <span className={styles.detail}>
                    {" "}
                    {item.kind === "folder" ? "Folder" : formatBytes(item.sizeBytes)} · {item.path}
                  </span>
                </span>
                <Button variant="quiet" onClick={() => void onRestore(item)}>
                  Restore<span className={styles.hidden}> {item.name}</span>
                </Button>
              </li>
            ))}
          </ul>
        ) : null}
      </section>

      {notice ? (
        <p className={styles.detail} role="status">
          {notice}
        </p>
      ) : null}
      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}
    </>
  );
}
