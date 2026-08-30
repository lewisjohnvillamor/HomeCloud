"use client";

import { useCallback, useEffect, useId, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { ErrorState, PendingState } from "@/components/ui/states";
import {
  fetchVersions,
  replaceContent,
  restoreVersion,
  versionContentUrl,
} from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { FileVersion, Item } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { formatBytes, formatDate } from "@/lib/format";
import styles from "@/components/share/share-dialog.module.css";

/**
 * What a file used to be, and putting it back.
 *
 * The empty state says plainly why there may be nothing here: HomeCloud
 * can only keep a version of a change it made itself, and a file edited
 * with another program was already changed before any scan saw it.
 */
export function VersionDialog({
  item,
  onClose,
  onChanged,
}: {
  item: Item;
  onClose: () => void;
  onChanged: () => void;
}) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const replaceInput = useRef<HTMLInputElement>(null);

  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchVersions(item.id, { signal }),
    [item.id],
  );
  const { state, reload } = useAsyncData<FileVersion[]>(load);

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

  async function onReplace(files: FileList | null) {
    const file = files?.[0];
    if (!file) {
      return;
    }

    setBusy(true);
    setProblem(null);

    const result = await replaceContent(item.id, file);
    if (result.ok) {
      onChanged();
      await reload();
    } else {
      setProblem(result.problem);
    }

    setBusy(false);
    if (replaceInput.current) {
      replaceInput.current.value = "";
    }
  }

  async function onRestore(version: FileVersion) {
    setBusy(true);
    setProblem(null);

    const result = await restoreVersion(item.id, version.id);
    if (result.ok) {
      onChanged();
      await reload();
    } else {
      setProblem(result.problem);
    }

    setBusy(false);
  }

  const versions = state.phase === "ready" ? state.data : null;

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
            History of “{item.name}”
          </h2>
          <Button variant="quiet" onClick={onClose} ref={closeButton}>
            Close
          </Button>
        </div>

        <p className={styles.detail}>
          Replacing this file here keeps what it was, so you can put it back. A file changed
          with another program is not kept: it had already changed before HomeCloud saw it.
        </p>

        <div className={styles.actions}>
          <input
            ref={replaceInput}
            className={styles.hidden}
            type="file"
            aria-label={`Replace ${item.name}`}
            onChange={(event) => void onReplace(event.target.files)}
          />
          <Button
            variant="primary"
            disabled={busy}
            onClick={() => replaceInput.current?.click()}
          >
            Replace contents
          </Button>
        </div>

        {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}
        {state.phase === "loading" ? <PendingState label="Loading history…" /> : null}
        {state.phase === "failed" ? (
          <ErrorState title="History could not be loaded" description={state.problem.detail} />
        ) : null}

        {versions && versions.length > 0 ? (
          <ul className={styles.list}>
            {versions.map((version) => (
              <li key={version.id} className={styles.share}>
                <span className={styles.shareMeta}>
                  {formatBytes(version.sizeBytes)} · replaced {formatDate(version.replacedAt)}
                </span>
                <span className={styles.actions}>
                  <a
                    className={styles.shareMeta}
                    href={versionContentUrl(item.id, version.id)}
                    download={item.name}
                  >
                    Download
                  </a>
                  <Button variant="quiet" disabled={busy} onClick={() => void onRestore(version)}>
                    Restore
                  </Button>
                </span>
              </li>
            ))}
          </ul>
        ) : null}

        {versions?.length === 0 ? (
          <p className={styles.shareMeta}>
            This file has no earlier contents kept.
          </p>
        ) : null}
      </div>
    </div>
  );
}
