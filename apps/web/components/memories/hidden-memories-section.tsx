"use client";

import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button";
import { ErrorState, PendingState } from "@/components/ui/states";
import { fetchHiddenMemories, unhideMemory } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import styles from "@/components/people/people-section.module.css";

/**
 * Memories that were dismissed, and the way back.
 *
 * Hiding something on the home screen is easy to do and easy to forget,
 * so the decision has to be findable again. Without this, "hide" is
 * indistinguishable from "delete" to the person who did it.
 */
export function HiddenMemoriesSection({ library }: { library: string }) {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchHiddenMemories(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<string[]>(load);

  async function onRestore(key: string) {
    setBusy(true);
    setProblem(null);

    const result = await unhideMemory(library, key);
    if (!result.ok) {
      setProblem(result.problem);
    }

    await reload();
    setBusy(false);
  }

  if (state.phase === "loading") {
    return <PendingState label="Loading hidden memories…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState title="Hidden memories could not be loaded" description={state.problem.detail} />
    );
  }

  if (state.data.length === 0) {
    return <p className={styles.meta}>Nothing hidden. Memories you dismiss appear here.</p>;
  }

  return (
    <>
      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}

      <ul className={styles.list}>
        {state.data.map((key) => (
          <li key={key} className={styles.row}>
            <span className={styles.name}>{describe(key)}</span>
            <Button variant="quiet" disabled={busy} onClick={() => void onRestore(key)}>
              Show again<span className={styles.hidden}> {describe(key)}</span>
            </Button>
          </li>
        ))}
      </ul>
    </>
  );
}

/**
 * A key in words. The keys are built to be stable, not readable, so this
 * turns them back into something a person can recognise.
 */
function describe(key: string): string {
  const onThisDay = /^on-this-day-(\d{2})-(\d{2})$/.exec(key);
  if (onThisDay?.[1] && onThisDay[2]) {
    const date = new Date(2000, Number(onThisDay[1]) - 1, Number(onThisDay[2]));
    return `On this day — ${date.toLocaleDateString(undefined, {
      day: "numeric",
      month: "long",
    })}`;
  }

  const trip = /^trip-(\d{4}-\d{2}-\d{2})-/.exec(key);
  if (trip?.[1]) {
    return `A trip — ${new Date(trip[1]).toLocaleDateString(undefined, {
      day: "numeric",
      month: "long",
      year: "numeric",
    })}`;
  }

  if (key === "recently-added") {
    return "Recently added";
  }

  return key;
}
