"use client";

import { useCallback, useMemo } from "react";
import { contentUrl, fetchPhotos, thumbnailUrl } from "@/lib/api/endpoints";
import type { Item } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import styles from "./photo-grid.module.css";

/** Photos grouped under the month they were taken. */
type Month = { key: string; label: string; photos: Item[] };

/**
 * Groups by month, newest first, with anything undated collected at the
 * end rather than guessed at. This is what makes a photo library read as
 * a timeline instead of an undifferentiated wall.
 */
function groupByMonth(photos: Item[]): Month[] {
  const months = new Map<string, Month>();

  for (const photo of photos) {
    const date = photo.modifiedAt ? new Date(photo.modifiedAt) : null;
    const valid = date && !Number.isNaN(date.getTime());

    const key = valid ? `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}` : "undated";
    const label = valid
      ? date.toLocaleDateString(undefined, { month: "long", year: "numeric" })
      : "No date";

    const month = months.get(key) ?? { key, label, photos: [] };
    month.photos.push(photo);
    months.set(key, month);
  }

  return [...months.values()].sort((a, b) => {
    if (a.key === "undated") return 1;
    if (b.key === "undated") return -1;

    return b.key.localeCompare(a.key);
  });
}

/**
 * The photo timeline, including videos.
 *
 * Tiles are generated thumbnails — a poster frame for a video — never
 * originals: a library of a few thousand items has to load on a phone
 * over a home network.
 */
export function PhotoGrid({ library }: { library: string }) {
  const load = useCallback(
    (signal: AbortSignal) => fetchPhotos(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<Item[]>(load);

  const photos = useMemo(() => (state.phase === "ready" ? state.data : []), [state]);
  const months = useMemo(() => groupByMonth(photos), [photos]);

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

      {months.map((month) => (
        <section key={month.key} className={styles.month} aria-labelledby={`month-${month.key}`}>
          <h2 id={`month-${month.key}`} className={styles.monthHeading}>
            {month.label}
            <span className={styles.monthCount}>{month.photos.length}</span>
          </h2>

          <ul className={styles.grid}>
            {month.photos.map((photo) => (
              <li key={photo.id}>
                <a
                  className={styles.tile}
                  href={contentUrl(photo.id)}
                  target="_blank"
                  rel="noreferrer"
                >
                  {/* eslint-disable-next-line @next/next/no-img-element -- the
                      optimizer cannot reach a private, session-protected origin. */}
                  <img
                    className={styles.image}
                    src={thumbnailUrl(photo.id, "small")}
                    srcSet={`${thumbnailUrl(photo.id, "small")} 1x, ${thumbnailUrl(photo.id, "medium")} 2x`}
                    alt={photo.name}
                    loading="lazy"
                    decoding="async"
                  />
                  {photo.isVideo ? (
                    <span className={styles.videoBadge} aria-hidden="true">
                      ▶
                    </span>
                  ) : null}
                  <span className={styles.caption}>
                    {photo.isVideo ? `Video · ${photo.name}` : photo.name}
                  </span>
                </a>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </>
  );
}
