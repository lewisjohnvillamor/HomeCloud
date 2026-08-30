"use client";

import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import { contentUrl, fetchMemories, hideMemory, thumbnailUrl } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { MemoryGroup } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import styles from "./memories.module.css";

/**
 * Memories on the home screen.
 *
 * Deterministic: what was taken on this day in earlier years, trips away
 * from home, and what arrived recently. No model is involved, which is
 * why this works with private AI switched off.
 *
 * Every memory can be dismissed. A memories engine that cannot be told
 * "not this one" eventually shows somebody a week they would rather not
 * be reminded of — and hiding hides the memory, never the photographs.
 */
export function MemoriesSection({ library }: { library: string }) {
  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(
    (signal: AbortSignal) => fetchMemories(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<MemoryGroup[]>(load);

  async function onHide(memory: MemoryGroup) {
    setBusy(true);
    setProblem(null);

    const result = await hideMemory(library, memory.key);
    if (!result.ok) {
      setProblem(result.problem);
    }

    await reload();
    setBusy(false);
  }

  if (state.phase === "loading") {
    return <PendingState label="Loading memories…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="Memories could not be loaded"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  if (state.data.length === 0) {
    return (
      <EmptyState
        title="No memories yet"
        description="Memories appear as photos build up — what you took on this day in other years, and trips away from home. You can bring back anything you have hidden from More."
      />
    );
  }

  return (
    <>
      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}

      {state.data.map((memory) => (
        <section key={memory.key} className={styles.memory} aria-labelledby={`m-${memory.key}`}>
          <div className={styles.header}>
            <h3 className={styles.title} id={`m-${memory.key}`}>
              {memory.title}
              <span className={styles.subtitle}>{memory.subtitle}</span>
            </h3>
            <Button variant="quiet" disabled={busy} onClick={() => void onHide(memory)}>
              <Icon name="close" />
              Hide
              <span className={styles.hidden}>
                {" "}
                {memory.title} {memory.subtitle}
              </span>
            </Button>
          </div>

          <ul className={styles.strip}>
            {memory.items.slice(0, 12).map((item) => (
              <li key={item.id}>
                <a
                  className={styles.tile}
                  href={contentUrl(item.id)}
                  target="_blank"
                  rel="noreferrer"
                  title={item.name}
                >
                  {/* eslint-disable-next-line @next/next/no-img-element */}
                  <img
                    className={styles.image}
                    src={thumbnailUrl(item.id, "small")}
                    alt={item.name}
                    loading="lazy"
                    decoding="async"
                  />
                </a>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </>
  );
}
