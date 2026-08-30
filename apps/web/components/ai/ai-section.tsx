"use client";

import { useCallback, useState } from "react";
import { ErrorState, PendingState } from "@/components/ui/states";
import { fetchAiSettings, updateAiSettings } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { AiSettings } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import styles from "./ai-section.module.css";

/**
 * The private AI switch.
 *
 * Two things are kept apart here, because conflating them is the failure
 * this feature has to avoid: what the owner asked for, and what this
 * machine can actually do. A server without the recogniser installed
 * says so, rather than accepting a setting and quietly doing nothing.
 *
 * Only what is built is offered. Photo understanding and face grouping
 * exist in the setting the server stores, but nothing implements them
 * yet, so putting them on screen would be a switch that lies.
 */
const CHOICES = [
  {
    profile: "off" as const,
    label: "Off",
    detail:
      "Nothing runs. Search still finds files by name and by the text inside documents, exactly as it does now.",
  },
  {
    profile: "text" as const,
    label: "Read text in pictures",
    detail:
      "Reads the words in scans, screenshots and photographs of documents, so you can search for what is written in them. Runs on the processor you already have — no graphics card, and nothing leaves this server.",
  },
];

export function AiSection({ library, isOwner }: { library: string; isOwner: boolean }) {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchAiSettings(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<AiSettings>(load);

  async function onChoose(profile: AiSettings["profile"]) {
    setBusy(true);
    setProblem(null);

    const result = await updateAiSettings(library, profile);
    if (!result.ok) {
      setProblem(result.problem);
    }

    await reload();
    setBusy(false);
  }

  if (state.phase === "loading") {
    return <PendingState label="Loading AI settings…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="AI settings could not be loaded"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  const settings = state.data;
  const installed = settings.ocrAvailable;

  return (
    <>
      {!installed ? (
        <p className={styles.notice}>
          This server cannot read text in pictures yet — the recogniser is not installed.
          Install <code className={styles.code}>tesseract-ocr</code> on the machine and
          restart HomeCloud. Everything else works without it.
        </p>
      ) : null}

      <ul className={styles.choices}>
        {CHOICES.map((choice) => {
          const chosen = settings.profile === choice.profile;
          const unavailable = choice.profile !== "off" && !installed;

          return (
            <li key={choice.profile}>
              <button
                type="button"
                className={styles.choice}
                aria-pressed={chosen ? "true" : "false"}
                data-chosen={chosen ? "true" : undefined}
                disabled={busy || !isOwner || unavailable}
                onClick={() => void onChoose(choice.profile)}
              >
                <span className={styles.choiceLabel}>{choice.label}</span>
                <span className={styles.choiceDetail}>{choice.detail}</span>
              </button>
            </li>
          );
        })}
      </ul>

      {settings.profile !== "off" && settings.pendingItems > 0 ? (
        <p className={styles.notice} role="status">
          {settings.pendingItems} {settings.pendingItems === 1 ? "picture" : "pictures"} still
          to read. They are read a hundred at a time after each scan, so this happens in the
          background and never holds up an upload.
        </p>
      ) : null}

      {!isOwner ? (
        <p className={styles.notice}>Only the library owner can change this.</p>
      ) : null}

      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}

      <p className={styles.notice}>
        Understanding what is <em>in</em> a photograph, and grouping the people in one, are
        still ahead. Both will be their own choice here rather than arriving switched on.
      </p>
    </>
  );
}
