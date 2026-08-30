"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  contentUrl,
  fetchMemories,
  fetchTvMemories,
  thumbnailUrl,
  tvContentUrl,
  tvThumbnailUrl,
} from "@/lib/api/endpoints";
import type { ApiResult } from "@/lib/api/client";
import type { Item, MemoryGroup } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { moveSelection, remoteAction } from "./tv-remote";
import styles from "./photo-wall.module.css";

/** Fallback used before the grid has been measured. */
const DEFAULT_COLUMNS = 5;

/** How long each photo stays on screen in the slideshow. */
const SLIDE_MS = 8000;

/**
 * How long each photo stays in photo-frame mode.
 *
 * Much slower than the slideshow, because nobody is watching this
 * deliberately. A frame that changes every eight seconds is a screen
 * demanding attention; one that changes every minute is a picture on a
 * shelf that happens to be different when you look up.
 */
const FRAME_MS = 60_000;

/**
 * Idle time before the frame starts on its own.
 *
 * A photo frame you have to switch on every morning is not a photo
 * frame. Five minutes is long enough that it never interrupts someone
 * actually using the remote.
 */
const IDLE_MS = 5 * 60_000;

/**
 * How often the clock moves, and how far.
 *
 * A television left on a shelf for months will keep whatever is drawn in
 * the same pixels. The clock drifts around the screen so nothing is
 * burned into one place.
 */
const DRIFT_MS = 4 * 60_000;
const DRIFT_POSITIONS = [
  { insetBlockStart: "8%", insetInlineStart: "8%" },
  { insetBlockStart: "8%", insetInlineEnd: "8%" },
  { insetBlockEnd: "12%", insetInlineEnd: "8%" },
  { insetBlockEnd: "12%", insetInlineStart: "8%" },
] as const;

/**
 * Where a photo wall gets its pictures.
 *
 * A television reaches the library one of two ways — signed in like any
 * other browser, or paired and holding a credential of its own — and the
 * wall itself should not care which. Both are expressed as the same
 * three questions.
 */
export type WallSource = {
  memories: (signal: AbortSignal) => Promise<ApiResult<MemoryGroup[]>>;
  thumbnail: (item: string) => string;
  content: (item: string) => string;
};

/** A signed-in browser showing the TV view. */
export function sessionSource(library: string): WallSource {
  return {
    memories: (signal) => fetchMemories(library, { signal }),
    thumbnail: (item) => thumbnailUrl(item, "medium"),
    content: (item) => contentUrl(item),
  };
}

/**
 * A screen someone paired, holding its own narrow credential.
 *
 * `onRevoked` fires when the server no longer recognises the token —
 * someone disconnected this screen from their phone. A television has
 * nobody standing in front of it to read an error, so the useful
 * response is to go back to showing a pairing code.
 */
export function pairedSource(token: string, onRevoked?: () => void): WallSource {
  return {
    memories: async (signal) => {
      const result = await fetchTvMemories(token, { signal });

      if (!result.ok && result.problem.code === "unauthenticated") {
        onRevoked?.();
      }

      return result;
    },
    thumbnail: (item) => tvThumbnailUrl(token, item),
    content: (item) => tvContentUrl(token, item),
  };
}

/**
 * The living-room view: a wall of photos, then a full-screen slideshow.
 *
 * Everything is driven from the four-direction remote model, and the
 * whole surface is one keyboard handler rather than a focus trap, so a
 * remote that sends only arrow keys still works.
 */
export function PhotoWall({ source }: { source: WallSource }) {
  const load = useCallback((signal: AbortSignal) => source.memories(signal), [source]);
  const { state } = useAsyncData<MemoryGroup[]>(load);

  const [selected, setSelected] = useState(0);
  const [playing, setPlaying] = useState<number | null>(null);
  const [paused, setPaused] = useState(false);
  // Photo-frame mode: ambient, slow, and no chrome at all.
  const [framing, setFraming] = useState(false);
  const [frame, setFrame] = useState(0);
  const [drift, setDrift] = useState(0);
  const [now, setNow] = useState<Date | null>(null);
  const wall = useRef<HTMLUListElement>(null);

  // Entering the frame sets the clock with it, so the first minute is
  // not blank and no effect has to write state as it runs.
  const enterFrame = useCallback(() => {
    setNow(new Date());
    setFraming(true);
  }, []);
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

      // Any key leaves the frame. Somebody has picked up the remote,
      // and whatever they want, it is not this.
      if (framing) {
        setFraming(false);
        return;
      }

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

      if (action === "playPause" && photos.length > 0) {
        enterFrame();
        return;
      }

      setSelected((current) => moveSelection(current, action, photos.length, columns));
    }

    window.addEventListener("keydown", onKeyDown);

    return () => window.removeEventListener("keydown", onKeyDown);
  }, [columns, enterFrame, framing, photos.length, playing, selected]);

  // Start the frame after a long enough silence, and restart the clock
  // whenever anything happens.
  useEffect(() => {
    if (framing || photos.length === 0) {
      return;
    }

    let timer = window.setTimeout(enterFrame, IDLE_MS);

    function restart() {
      window.clearTimeout(timer);
      timer = window.setTimeout(enterFrame, IDLE_MS);
    }

    const events = ["keydown", "pointerdown", "pointermove"] as const;
    for (const event of events) {
      window.addEventListener(event, restart);
    }

    return () => {
      window.clearTimeout(timer);
      for (const event of events) {
        window.removeEventListener(event, restart);
      }
    };
  }, [enterFrame, framing, photos.length]);

  // The frame's own slow advance, its clock, and the drift that keeps
  // the clock from burning into one corner.
  useEffect(() => {
    if (!framing || photos.length === 0) {
      return;
    }

    const advance = window.setInterval(
      () => setFrame((current) => (current + 1) % photos.length),
      FRAME_MS,
    );
    const clock = window.setInterval(() => setNow(new Date()), 30_000);
    const drifting = window.setInterval(
      () => setDrift((current) => (current + 1) % DRIFT_POSITIONS.length),
      DRIFT_MS,
    );

    return () => {
      window.clearInterval(advance);
      window.clearInterval(clock);
      window.clearInterval(drifting);
    };
  }, [framing, photos.length]);

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
  const framed = framing ? photos[frame % photos.length] : null;

  if (framed) {
    return (
      <div className={styles.frame} role="img" aria-label={`Photo frame: ${framed.name}`}>
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img className={styles.frameImage} src={source.content(framed.id)} alt={framed.name} />

        {/* Low contrast and drifting: a clock on a shelf, not a caption,
            and never in the same pixels long enough to burn in. */}
        <p className={styles.frameClock} style={DRIFT_POSITIONS[drift]}>
          {now
            ? now.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })
            : ""}
        </p>
      </div>
    );
  }

  return (
    <>
      <h1 className={styles.title}>Photos</h1>
      <p className={styles.hint}>
        Arrows to move · Enter to play · Play/pause for the photo frame · Escape to go back
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
                      src={source.thumbnail(photo.id)}
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
          <img className={styles.slide} src={source.content(current.id)} alt={current.name} />
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
