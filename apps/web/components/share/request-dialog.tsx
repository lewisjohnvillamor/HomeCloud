"use client";

import { useEffect, useId, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { ErrorState } from "@/components/ui/states";
import { createUploadRequest } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { Item, UploadRequest } from "@/lib/api/types";
import styles from "./share-dialog.module.css";

/** Expiry choices, in days. `null` means "until I revoke it". */
const EXPIRY_CHOICES: { label: string; days: number | null }[] = [
  { label: "7 days", days: 7 },
  { label: "30 days", days: 30 },
  { label: "1 year", days: 365 },
  { label: "Until I revoke it", days: null },
];

/**
 * Asking someone for files.
 *
 * The mirror image of the share dialog, and the copy says so plainly:
 * this hands out writing, not reading, and the person who gets it will
 * not see what is already in the folder.
 */
export function RequestDialog({ item, onClose }: { item: Item; onClose: () => void }) {
  const titleId = useId();
  const messageId = useId();
  const expiryId = useId();
  const panel = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);

  const [message, setMessage] = useState(`Send files to ${item.name}`);
  const [expiry, setExpiry] = useState<string>("30");
  const [created, setCreated] = useState<UploadRequest | null>(null);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  useEffect(() => {
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

    const result = await createUploadRequest(item.id, {
      title: message.trim() || undefined,
      expiresInDays: expiry === "never" ? null : Number(expiry),
    });

    if (result.ok) {
      setCreated(result.data);
    } else {
      setProblem(result.problem);
    }

    setBusy(false);
  }

  const link = created?.token ? `${window.location.origin}/u/${created.token}` : "";

  async function onCopy() {
    try {
      await navigator.clipboard.writeText(link);
      setCopied(true);
    } catch {
      // The link is on screen and selectable; a refused clipboard is not
      // worth interrupting for.
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
            Ask for files in “{item.name}”
          </h2>
          <Button variant="quiet" onClick={onClose} ref={closeButton}>
            Close
          </Button>
        </div>

        <p className={styles.detail}>
          Anyone with this link can <strong>send</strong> files into this folder. They cannot
          see what is already in it, cannot reach anything else in your library, and cannot
          take anything back out.
        </p>

        <div className={styles.field}>
          <label className={styles.fieldLabel} htmlFor={messageId}>
            What to call it
          </label>
          <input
            id={messageId}
            className={styles.link}
            value={message}
            onChange={(event) => setMessage(event.target.value)}
            maxLength={96}
          />
          <span className={styles.shareMeta}>Shown to whoever opens the link.</span>
        </div>

        <div className={styles.expiry}>
          <label htmlFor={expiryId}>Link expires after</label>
          <select
            id={expiryId}
            className={styles.select}
            value={expiry}
            onChange={(event) => setExpiry(event.target.value)}
          >
            {EXPIRY_CHOICES.map((choice) => (
              <option
                key={choice.label}
                value={choice.days === null ? "never" : String(choice.days)}
              >
                {choice.label}
              </option>
            ))}
          </select>
        </div>

        {created?.token ? (
          <>
            <p className={styles.detail}>
              Copy this link now — it is not shown again. It accepts up to{" "}
              {created.maxFiles} files.
            </p>
            <div className={styles.linkRow}>
              <input
                className={styles.link}
                readOnly
                value={link}
                aria-label="Upload link"
                onFocus={(event) => event.target.select()}
              />
              <Button onClick={() => void onCopy()}>{copied ? "Copied" : "Copy"}</Button>
            </div>
          </>
        ) : (
          <div className={styles.actions}>
            <Button variant="primary" onClick={() => void onCreate()} disabled={busy}>
              Create link
            </Button>
          </div>
        )}

        {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}
      </div>
    </div>
  );
}
