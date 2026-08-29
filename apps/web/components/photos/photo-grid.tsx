"use client";

import { useCallback } from "react";
import { contentUrl, fetchPhotos } from "@/lib/api/endpoints";
import type { Item } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import styles from "./photo-grid.module.css";

/**
 * The photo timeline.
 *
 * Full-size originals are shown, downscaled by the browser: thumbnails
 * need a derivative pipeline with its own resource limits, and inventing
 * one here would be worse than waiting for it.
 */
export function PhotoGrid({ library }: { library: string }) {
  const load = useCallback(
    (signal: AbortSignal) => fetchPhotos(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<Item[]>(load);

  if (state.phase === "loading") {
    return <PendingState label="Loading photos…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="Photos could not be loaded"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  const photos = state.data;

  if (photos.length === 0) {
    return (
      <EmptyState
        title="No photos yet"
        description="Photos appear here once images are in the library. Upload some from Files, or run a scan from More."
      />
    );
  }

  return (
    <>
      <p className={styles.note}>
        {photos.length} photo{photos.length === 1 ? "" : "s"}
      </p>
      <ul className={styles.grid}>
        {photos.map((photo) => (
          <li key={photo.id}>
            <div className={styles.tile}>
              <a className={styles.link} href={contentUrl(photo.id)} target="_blank" rel="noreferrer">
                {/* eslint-disable-next-line @next/next/no-img-element -- the
                    optimizer cannot reach a private, session-protected origin. */}
                <img
                  className={styles.image}
                  src={contentUrl(photo.id)}
                  alt={photo.name}
                  loading="lazy"
                  decoding="async"
                />
              </a>
            </div>
            <span className={styles.caption} title={photo.path}>
              {photo.name}
            </span>
          </li>
        ))}
      </ul>
    </>
  );
}
