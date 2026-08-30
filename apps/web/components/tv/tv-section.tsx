"use client";

import { useCallback, useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";
import { ErrorState, PendingState } from "@/components/ui/states";
import { fetchTvDevices, unpairTvDevice } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { TvDevice } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { formatDate } from "@/lib/format";
import styles from "@/components/people/people-section.module.css";
import local from "./tv-section.module.css";

/**
 * Televisions paired with this library.
 *
 * A screen in a shared room is exactly the thing someone should be able
 * to see a list of and switch off, so the list says when each was
 * connected and when it last asked for anything.
 */
export function TvSection({ library }: { library: string }) {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchTvDevices(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<TvDevice[]>(load);

  async function onUnpair(device: TvDevice) {
    setBusy(true);
    setProblem(null);

    const result = await unpairTvDevice(device.id);
    if (!result.ok) {
      setProblem(result.problem);
    }

    await reload();
    setBusy(false);
  }

  return (
    <>
      {state.phase === "loading" ? <PendingState label="Loading screens…" /> : null}
      {state.phase === "failed" ? (
        <ErrorState title="Screens could not be loaded" description={state.problem.detail} />
      ) : null}

      {state.phase === "ready" && state.data.length === 0 ? (
        <p className={styles.meta}>
          No televisions connected. Open HomeCloud on a TV and{" "}
          <Link href="/pair">connect it</Link> with the code it shows.
        </p>
      ) : null}

      {state.phase === "ready" && state.data.length > 0 ? (
        <ul className={styles.list}>
          {state.data.map((device) => (
            <li key={device.id} className={styles.row}>
              <span>
                <span className={`${styles.name} ${local.name}`}>{device.name}</span>
                <span className={styles.meta}>
                  Connected {formatDate(device.createdAt)} ·{" "}
                  {device.lastSeenAt
                    ? `last used ${formatDate(device.lastSeenAt)}`
                    : "not used yet"}
                </span>
              </span>
              <Button variant="quiet" onClick={() => void onUnpair(device)} disabled={busy}>
                Disconnect<span className={styles.hidden}> {device.name}</span>
              </Button>
            </li>
          ))}
        </ul>
      ) : null}

      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}
    </>
  );
}
