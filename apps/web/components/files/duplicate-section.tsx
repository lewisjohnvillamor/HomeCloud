"use client";

import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button";
import { ErrorState, PendingState } from "@/components/ui/states";
import { fetchDuplicates, trashItem } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { DuplicateGroup, Item } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { formatBytes } from "@/lib/format";
import styles from "@/components/people/people-section.module.css";

/**
 * Files that are byte-for-byte the same.
 *
 * Exact matches only. Files that merely look alike are a job for the AI
 * half of the product; calling anything less than identical a duplicate
 * would invite someone to delete a photo they wanted.
 *
 * Nothing is removed automatically. The extras are listed and a person
 * decides, because the one thing worse than three copies of a photo is
 * none.
 */
export function DuplicateSection({ library }: { library: string }) {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchDuplicates(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<DuplicateGroup[]>(load);

  async function onTrash(item: Item) {
    if (!window.confirm(`Move “${item.name}” to the trash? The other copies stay.`)) {
      return;
    }

    setBusy(true);
    setProblem(null);

    const result = await trashItem(item.id);
    if (!result.ok) {
      setProblem(result.problem);
    }

    await reload();
    setBusy(false);
  }

  if (state.phase === "loading") {
    return <PendingState label="Looking for duplicates…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="Duplicates could not be loaded"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  if (state.data.length === 0) {
    return (
      <p className={styles.meta}>
        No exact duplicates found. Files are hashed in the background after a scan, so a
        library that was just added may take a few scans to finish checking.
      </p>
    );
  }

  const reclaimable = state.data.reduce((total, group) => total + group.reclaimableBytes, 0);

  return (
    <>
      <p className={styles.meta}>
        {state.data.length} {state.data.length === 1 ? "set" : "sets"} of identical files.
        Removing the extras would free {formatBytes(reclaimable)}.
      </p>

      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}

      <ul className={styles.list}>
        {state.data.map((group) => (
          <li key={group.items[0]?.id ?? ""} className={styles.row}>
            <span>
              <span className={styles.name}>
                {group.items.length} copies · {formatBytes(group.sizeBytes)} each
              </span>
              <ul className={styles.list}>
                {group.items.map((item, index) => (
                  <li key={item.id} className={styles.meta}>
                    {item.path}
                    {index === 0 ? (
                      // The oldest copy is almost always the one someone
                      // means to keep, so it is named rather than
                      // offered for deletion first.
                      <> — oldest</>
                    ) : (
                      <>
                        {" "}
                        <Button variant="quiet" disabled={busy} onClick={() => void onTrash(item)}>
                          Move to trash
                          <span className={styles.hidden}> {item.path}</span>
                        </Button>
                      </>
                    )}
                  </li>
                ))}
              </ul>
            </span>
          </li>
        ))}
      </ul>
    </>
  );
}
