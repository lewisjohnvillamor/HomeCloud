"use client";

import { useCallback, useMemo } from "react";
import { fetchPhotos } from "@/lib/api/endpoints";
import type { Item } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import { PhotoTile } from "./photo-tile";
import styles from "./photo-grid.module.css";

/**
 * Says what is actually here rather than calling everything a photo: a
 * timeline of eleven pictures and one clip is not "12 photos".
 */
function describe(items: Item[]): string {
  const videos = items.filter((item) => item.isVideo).length;
  const photos = items.length - videos;

  const parts: string[] = [];
  if (photos > 0) {
    parts.push(`${photos} photo${photos === 1 ? "" : "s"}`);
  }
  if (videos > 0) {
    parts.push(`${videos} video${videos === 1 ? "" : "s"}`);
  }

  return parts.join(" · ");
}

/** Photos grouped under the month they were taken. */
type Month = { key: string; label: string; photos: Item[] };

/**
 * Groups by month, newest first, with anything undated collected at the
 * end rather than guessed at. This is what makes a photo library read as
 * a timeline instead of an undifferentiated wall.
 *
 * The camera's date wins over the file's: copying a folder of holiday
 * photos to a new disk rewrites every file time, and a timeline that
 * puts a decade of pictures under this month is no timeline at all.
 */
function groupByMonth(photos: Item[]): Month[] {
  const months = new Map<string, Month>();

  for (const photo of photos) {
    const taken = photo.takenAt ?? photo.modifiedAt;
    const date = taken ? new Date(taken) : null;
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
export function PhotoGrid({
  library,
  selecting,
  selected,
  favorites,
  onToggleSelected,
  onToggleFavorite,
}: {
  library: string;
  selecting?: boolean;
  selected?: ReadonlySet<string>;
  favorites?: ReadonlySet<string>;
  onToggleSelected?: (photo: Item) => void;
  onToggleFavorite?: (photo: Item) => void;
}) {
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
      <p className={styles.note}>{describe(photos)}</p>

      {months.map((month) => (
        <section key={month.key} className={styles.month} aria-labelledby={`month-${month.key}`}>
          <h2 id={`month-${month.key}`} className={styles.monthHeading}>
            {month.label}
            <span className={styles.monthCount}>{month.photos.length}</span>
          </h2>

          <ul className={styles.grid}>
            {month.photos.map((photo) => (
              <PhotoTile
                key={photo.id}
                photo={photo}
                selecting={selecting}
                selected={selected?.has(photo.id)}
                favorite={favorites?.has(photo.id)}
                onToggleSelected={onToggleSelected}
                onToggleFavorite={onToggleFavorite}
              />
            ))}
          </ul>
        </section>
      ))}
    </>
  );
}
