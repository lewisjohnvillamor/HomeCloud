"use client";

import { useCallback, useEffect, useId, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { ErrorState, PendingState } from "@/components/ui/states";
import { createShare, fetchSharesForItem, revokeShare } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { Item, Share } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { formatDate } from "@/lib/format";
import styles from "./share-dialog.module.css";

/** Mirrors the server's minimum so the message arrives before a round trip. */
const MIN_SHARE_PASSWORD_LENGTH = 8;

/** Expiry choices, in days. `null` means "until I revoke it". */
const EXPIRY_CHOICES: { label: string; days: number | null }[] = [
  { label: "7 days", days: 7 },
  { label: "30 days", days: 30 },
  { label: "1 year", days: 365 },
  { label: "Until I revoke it", days: null },
];

/**
 * Creating and revoking public links for one item.
 *
 * A link is read-only and points at this item alone — the copy says so,
 * because a person deciding whether to send a link needs to know what
 * they are handing over.
 */
export function ShareDialog({ item, onClose }: { item: Item; onClose: () => void }) {
  const titleId = useId();
  const expiryId = useId();
  const passwordId = useId();
  const panel = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);

  const [created, setCreated] = useState<Share | null>(null);
  const [expiry, setExpiry] = useState<string>("7");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchSharesForItem(item.id, { signal }),
    [item.id],
  );
  const { state, reload } = useAsyncData<Share[]>(load);
  const shares = state.phase === "ready" ? state.data : null;

  useEffect(() => {
    // Focus moves into the dialog so a keyboard user is not left behind
    // on the page underneath.
    closeButton.current?.focus();
  }, []);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
      }
    }

    document.addEventListener("keydown", onKeyDown);

    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  async function onCreate() {
    setBusy(true);
    setProblem(null);
    setCopied(false);

    const days = expiry === "never" ? null : Number(expiry);
    const secret = password.trim();

    if (secret.length > 0 && secret.length < MIN_SHARE_PASSWORD_LENGTH) {
      setProblem({
        code: "bad_request",
        detail: `A link password needs at least ${MIN_SHARE_PASSWORD_LENGTH} characters.`,
      });
      setBusy(false);
      return;
    }

    const result = await createShare(item.id, days, secret.length > 0 ? secret : null);

    if (result.ok) {
      setCreated(result.data);
      setPassword("");
      await reload();
    } else {
      setProblem(result.problem);
    }

    setBusy(false);
  }

  async function onRevoke(share: Share) {
    setBusy(true);
    const result = await revokeShare(share.id);

    if (!result.ok) {
      setProblem(result.problem);
    }
    if (created?.id === share.id) {
      setCreated(null);
    }

    await reload();
    setBusy(false);
  }

  const linkFor = (share: Share) =>
    share.token ? `${window.location.origin}/s/${share.token}` : "";

  async function onCopy(link: string) {
    try {
      await navigator.clipboard.writeText(link);
      setCopied(true);
    } catch {
      // Clipboard access can be refused; the link is on screen and
      // selectable, so this is not an error worth interrupting for.
      setCopied(false);
    }
  }

  return (
    <div
      className={styles.dialog}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      onMouseDown={(event) => {
        if (!panel.current?.contains(event.target as Node)) {
          onClose();
        }
      }}
    >
      <div className={styles.panel} ref={panel}>
        <div className={styles.header}>
          <h2 className={styles.title} id={titleId}>
            Share “{item.name}”
          </h2>
          <Button variant="quiet" onClick={onClose} ref={closeButton}>
            Close
          </Button>
        </div>

        <p className={styles.detail}>
          A link gives read-only access to this {item.kind === "folder" ? "folder" : "file"}{" "}
          and nothing else in your library. Anyone with the link can open it.
        </p>

        <div className={styles.expiry}>
          <label htmlFor={expiryId}>Link expires after</label>
          <select
            id={expiryId}
            className={styles.select}
            value={expiry}
            onChange={(event) => setExpiry(event.target.value)}
          >
            {EXPIRY_CHOICES.map((choice) => (
              <option key={choice.label} value={choice.days === null ? "never" : String(choice.days)}>
                {choice.label}
              </option>
            ))}
          </select>
        </div>

        <div className={styles.field}>
          <label className={styles.fieldLabel} htmlFor={passwordId}>
            Password (optional)
          </label>
          <input
            id={passwordId}
            className={styles.link}
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            autoComplete="off"
            placeholder="Leave empty for no password"
          />
          <span className={styles.shareMeta}>
            Add one when the link travels somewhere you do not fully trust. Send
            it separately from the link itself.
          </span>
        </div>

        {created?.token ? (
          <>
            <p className={styles.detail}>
              Copy this link now — it is not shown again.
              {created.protected ? " Whoever opens it will be asked for the password." : ""}
            </p>
            <div className={styles.linkRow}>
              <input
                className={styles.link}
                readOnly
                value={linkFor(created)}
                aria-label="Share link"
                onFocus={(event) => event.target.select()}
              />
              <Button onClick={() => void onCopy(linkFor(created))}>
                {copied ? "Copied" : "Copy"}
              </Button>
            </div>
          </>
        ) : null}

        <div className={styles.actions}>
          <Button variant="primary" onClick={() => void onCreate()} disabled={busy}>
            Create link
          </Button>
        </div>

        {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}
        {state.phase === "failed" ? (
          <ErrorState title="Links could not be loaded" description={state.problem.detail} />
        ) : null}
        {state.phase === "loading" ? <PendingState label="Loading links…" /> : null}

        {shares && shares.length > 0 ? (
          <ul className={styles.list}>
            {shares.map((share) => (
              <li key={share.id} className={styles.share}>
                <span>
                  <span className={styles.shareMeta}>
                    Created {formatDate(share.createdAt)} ·{" "}
                    {share.expiresAt ? `expires ${formatDate(share.expiresAt)}` : "no expiry"} ·{" "}
                    opened {share.accessCount} time{share.accessCount === 1 ? "" : "s"}
                    {share.protected ? " · password required" : ""}
                  </span>
                </span>
                <Button variant="quiet" onClick={() => void onRevoke(share)} disabled={busy}>
                  Revoke
                </Button>
              </li>
            ))}
          </ul>
        ) : null}

        {shares?.length === 0 ? (
          <p className={styles.shareMeta}>No links for this item yet.</p>
        ) : null}
      </div>
    </div>
  );
}
