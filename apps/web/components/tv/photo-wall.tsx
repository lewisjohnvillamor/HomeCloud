"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { contentUrl, fetchMemories, thumbnailUrl } from "@/lib/api/endpoints";
import type { Item, MemoryGroup } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { moveSelection, remoteAction } from "./tv-remote";
import styles from "./photo-wall.module.css";

/** Fallback used before the grid has been measured. */
const DEFAULT_COLUMNS = 5;

/** How long each photo stays on screen in the slideshow. */
const SLIDE_MS = 8000;

/**
 * The living-room view: a wall of photos, then a full-screen slideshow.
 *
 * Everything is driven from the four-direction remote model, and the
 * whole surface is one keyboard handler rather than a focus trap, so a
 * remote that sends only arrow keys still works.
 */
export function PhotoWall({ library }: { library: string }) {
  const load = useCallback(
    (signal: AbortSignal) => fetchMemories(library, { signal }),
    [library],
  );
  const { state } = useAsyncData<MemoryGroup[]>(load);

  const [selected, setSelected] = useState(0);
  const [playing, setPlaying] = useState<number | null>(null);
  const [paused, setPaused] = useState(false);
  const wall = useRef<HTMLUListElement>(null);
  const columns = useGridColumns(wall);

  const groups = useMemo(() => (state.phase === "ready" ? state.data : []), [state]);
  // One flat list: the remote moves through photos, not through sections.
  const photos: Item[] = useMemo(
    () => groups.flatMap((group) => group.items),
    [groups],
  );
  // Position of each photo in that list, so a tile does not have to scan
  // the whole wall to find its own index.
  const positions = useMemo(() => {
    const positions = new Map<string, number>();
    photos.forEach((photo, index) => positions.set(photo.id, index));

    return positions;
  }, [photos]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const action = remoteAction(event);
      if (!action) {
        return;
      }

      event.preventDefault();

      if (playing !== null) {
        switch (action) {
          case "left":
            setPlaying((current) => Math.max(0, (current ?? 0) - 1));
            return;
          case "right":
            setPlaying((current) => Math.min(photos.length - 1, (current ?? 0) + 1));
            return;
          case "back":
            setPlaying(null);
            setPaused(false);
            return;
          case "select":
          case "playPause":
            setPaused((current) => !current);
            return;
          default:
            return;
        }
      }

      if (action === "select") {
        if (photos.length > 0) {
          setPlaying(selected);
        }
        return;
      }

      setSelected((current) => moveSelection(current, action, photos.length, columns));
    }

    window.addEventListener("keydown", onKeyDown);

    return () => window.removeEventListener("keydown", onKeyDown);
  }, [columns, photos.length, playing, selected]);

  // Auto-advance, unless paused or the viewer asked for less motion.
  useEffect(() => {
    if (playing === null || paused || photos.length === 0) {
      return;
    }
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      return;
    }

    const timer = setTimeout(() => {
      setPlaying((current) => ((current ?? 0) + 1) % photos.length);
    }, SLIDE_MS);

    return () => clearTimeout(timer);
  }, [playing, paused, photos.length]);

  // Keep the selected tile on screen as the remote moves down the wall.
  useEffect(() => {
    document
      .querySelector(`[data-tile="${selected}"]`)
      ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [selected]);

  if (state.phase === "loading") {
    return <p className={styles.message}>Loading your photos…</p>;
  }

  if (state.phase === "failed") {
    return (
      <p className={styles.message} role="alert">
        {state.problem.detail}
      </p>
    );
  }

  if (photos.length === 0) {
    return (
      <div className={styles.empty}>
        <h1 className={styles.title}>Nothing to show yet</h1>
        <p className={styles.message}>
          Add photos from a phone or computer, then run a scan. They appear here
          automatically.
        </p>
      </div>
    );
  }

  const current = playing === null ? null : photos[playing];

  return (
    <>
      <h1 className={styles.title}>Photos</h1>
      <p className={styles.hint}>
        Arrows to move · Enter to play · Escape to go back
      </p>

      {groups.map((group) => (
        <section key={group.title} className={styles.group}>
          <h2 className={styles.groupTitle}>
            {group.title}
            <span className={styles.groupSubtitle}>{group.subtitle}</span>
          </h2>

          <ul className={styles.wall} ref={wall}>
            {group.items.map((photo) => {
              const index = positions.get(photo.id) ?? 0;
              const isSelected = index === selected;

              return (
                <li key={`${group.title}-${photo.id}`}>
                  <button
                    type="button"
                    data-tile={index}
                    className={styles.tile}
                    data-selected={isSelected ? "true" : undefined}
                    aria-current={isSelected ? "true" : undefined}
                    onClick={() => {
                      setSelected(index);
                      setPlaying(index);
                    }}
                  >
                    {/* eslint-disable-next-line @next/next/no-img-element */}
                    <img
                      className={styles.thumb}
                      src={thumbnailUrl(photo.id, "medium")}
                      alt={photo.name}
                      loading="lazy"
                      decoding="async"
                    />
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      ))}

      {current ? (
        <div className={styles.slideshow} role="dialog" aria-modal="true" aria-label="Slideshow">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img className={styles.slide} src={contentUrl(current.id)} alt={current.name} />
          <div className={styles.slideBar}>
            <p className={styles.slideName}>{current.name}</p>
            <p className={styles.slideHint}>
              {paused ? "Paused" : "Playing"} · {playing! + 1} of {photos.length} · Enter to{" "}
              {paused ? "resume" : "pause"} · Escape to go back
            </p>
          </div>
          <p className={styles.live} role="status" aria-live="polite">
            {current.name}
          </p>
        </div>
      ) : null}
    </>
  );
}

/**
 * How many tiles the grid is actually showing per row.
 *
 * Measured rather than assumed: the remote moves up and down by whole
 * rows, so if the layout narrows and the code does not notice, the
 * cursor jumps to the wrong photo.
 */
function useGridColumns(grid: React.RefObject<HTMLElement | null>): number {
  const [columns, setColumns] = useState(DEFAULT_COLUMNS);

  useEffect(() => {
    const element = grid.current;
    if (!element || typeof ResizeObserver === "undefined") {
      return;
    }

    const measure = () => {
      const template = window.getComputedStyle(element).gridTemplateColumns;
      const count = template.split(" ").filter((value) => value.trim().length > 0).length;

      setColumns(count > 0 ? count : DEFAULT_COLUMNS);
    };

    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);

    return () => observer.disconnect();
  }, [grid]);

  return columns;
}
