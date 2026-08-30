"use client";

import { useCallback, useMemo, useState } from "react";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import { fetchPlaces, thumbnailUrl, contentUrl } from "@/lib/api/endpoints";
import type { Item } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import styles from "./places.module.css";

/** One axis label: "51.5°N", "0.1°W". */
function degrees(value: number, positive: string, negative: string): string {
  return `${Math.abs(value).toFixed(1)}°${value < 0 ? negative : positive}`;
}

/**
 * Where photos were taken.
 *
 * A coordinate plot, not a map, and it says so on the page. A
 * self-hosted server should not tell a third party where its owner's
 * photos were taken, and asking a tile service for the squares around a
 * coordinate does exactly that — so there is no tile service, no
 * account, and no request leaving the house.
 *
 * What that costs is the coastline: this shows the photos' positions
 * relative to each other with the degrees marked, which answers "were
 * these taken in the same place?" and "roughly where?" but not "which
 * street?". Drawing a crude world outline from memory would look like a
 * map while being wrong, which is worse than plainly being a plot.
 */
export function PlacesView({ library }: { library: string }) {
  const [selected, setSelected] = useState<string | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchPlaces(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<Item[]>(load);

  const photos = useMemo(() => (state.phase === "ready" ? state.data : []), [state]);

  /** The area the photos cover, padded so nothing sits on the edge. */
  const bounds = useMemo(() => {
    if (photos.length === 0) {
      return null;
    }

    const latitudes = photos.map((photo) => photo.latitude ?? 0);
    const longitudes = photos.map((photo) => photo.longitude ?? 0);

    // A minimum span, or a single photo would be infinitely zoomed.
    const pad = 2;
    return {
      south: Math.max(-90, Math.min(...latitudes) - pad),
      north: Math.min(90, Math.max(...latitudes) + pad),
      west: Math.max(-180, Math.min(...longitudes) - pad),
      east: Math.min(180, Math.max(...longitudes) + pad),
    };
  }, [photos]);

  if (state.phase === "loading") {
    return <PendingState label="Loading places…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="Places could not be loaded"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  if (photos.length === 0 || !bounds) {
    return (
      <EmptyState
        title="No photos with a place"
        description="Photos appear here when the camera recorded where they were taken. Many cameras have this switched off, and a photo sent through a messaging app usually has it stripped."
      />
    );
  }

  const width = bounds.east - bounds.west || 1;
  const height = bounds.north - bounds.south || 1;

  /** Longitude and latitude to a 0–100 position in the plot. */
  function position(photo: Item) {
    return {
      x: (((photo.longitude ?? 0) - bounds!.west) / width) * 100,
      // Latitude runs the other way from screen coordinates.
      y: ((bounds!.north - (photo.latitude ?? 0)) / height) * 100,
    };
  }

  const chosen = photos.find((photo) => photo.id === selected) ?? null;

  return (
    <>
      <p className={styles.note}>
        {photos.length} {photos.length === 1 ? "photo" : "photos"} recorded where they were
        taken, plotted by latitude and longitude. No map service is asked where your photos
        are, and a shared link never carries a location.
      </p>

      <div className={styles.frame}>
        <span className={`${styles.axis} ${styles.north}`}>{degrees(bounds.north, "N", "S")}</span>
        <span className={`${styles.axis} ${styles.south}`}>{degrees(bounds.south, "N", "S")}</span>
        <span className={`${styles.axis} ${styles.west}`}>{degrees(bounds.west, "E", "W")}</span>
        <span className={`${styles.axis} ${styles.east}`}>{degrees(bounds.east, "E", "W")}</span>

        <div className={styles.plot} role="group" aria-label="Photo locations">
        {photos.map((photo) => {
          const { x, y } = position(photo);

          return (
            <button
              key={photo.id}
              type="button"
              className={styles.pin}
              style={{ left: `${x}%`, top: `${y}%` }}
              aria-pressed={photo.id === selected ? "true" : "false"}
              onClick={() => setSelected(photo.id === selected ? null : photo.id)}
            >
              <span className={styles.hidden}>
                {photo.name} at {(photo.latitude ?? 0).toFixed(3)},{" "}
                {(photo.longitude ?? 0).toFixed(3)}
              </span>
            </button>
          );
        })}
        </div>
      </div>

      {chosen ? (
        <a className={styles.chosen} href={contentUrl(chosen.id)} target="_blank" rel="noreferrer">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            className={styles.chosenImage}
            src={thumbnailUrl(chosen.id, "medium")}
            alt=""
            loading="lazy"
          />
          <span>
            <span className={styles.chosenName}>{chosen.name}</span>
            <span className={styles.chosenPlace}>
              {(chosen.latitude ?? 0).toFixed(4)}, {(chosen.longitude ?? 0).toFixed(4)}
            </span>
          </span>
        </a>
      ) : (
        <p className={styles.note}>Choose a point to see the photo taken there.</p>
      )}
    </>
  );
}
