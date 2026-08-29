"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiResult } from "@/lib/api/client";
import type { ApiProblem } from "@/lib/api/problem";

export type AsyncState<T> =
  | { phase: "loading" }
  | { phase: "ready"; data: T }
  | { phase: "failed"; problem: ApiProblem };

/**
 * Loads data for a view, with cancellation and a reload handle.
 *
 * One place owns this pattern so the abort handling, the "ignore a
 * response that arrived after the inputs changed" rule, and the lint
 * exemption for kicking off a fetch from an effect all live together
 * rather than being repeated in every component.
 */
export function useAsyncData<T>(
  load: (signal: AbortSignal) => Promise<ApiResult<T>>,
): { state: AsyncState<T>; reload: () => Promise<void> } {
  const [state, setState] = useState<AsyncState<T>>({ phase: "loading" });

  // Identifies the most recent request, so a slow earlier response
  // cannot overwrite a newer one.
  const generation = useRef(0);

  const run = useCallback(
    async (signal: AbortSignal, showPending: boolean) => {
      generation.current += 1;
      const current = generation.current;

      if (showPending) {
        setState({ phase: "loading" });
      }

      const result = await load(signal);

      if (signal.aborted || current !== generation.current) {
        return;
      }

      setState(
        result.ok
          ? { phase: "ready", data: result.data }
          : { phase: "failed", problem: result.problem },
      );
    },
    [load],
  );

  useEffect(() => {
    const controller = new AbortController();

    // This is the "subscribe to an external system" case: the fetch
    // starts here and state is set from its callback, never
    // synchronously during the effect body.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void run(controller.signal, false);

    return () => controller.abort();
  }, [run]);

  const reload = useCallback(async () => {
    const controller = new AbortController();
    await run(controller.signal, false);
  }, [run]);

  return { state, reload };
}
