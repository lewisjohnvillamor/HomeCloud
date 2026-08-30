"use client";

import type { Item } from "@/lib/api/types";
import { contentUrl, thumbnailUrl } from "@/lib/api/endpoints";
import { formatDate } from "@/lib/format";
import styles from "./photo-grid.module.css";

/**
 * What a photo says about itself, for the tile's hover text: the day it
 * was taken and what took it, when the camera recorded either.
 */
export function details(photo: Item): string {
  const parts = [photo.name];

  if (photo.takenAt) {
    parts.push(formatDate(photo.takenAt));
  }
  if (photo.camera) {
    parts.push(photo.camera);
  }

  return parts.join(" · ");
}

/**
 * One picture in a grid.
 *
 * Two modes, because a photo grid is used two ways: normally a tile is a
 * link that opens the original, and while a selection is being made it
 * is a checkbox. Making that one component keeps the two from drifting
 * apart in size, focus behaviour, or what the caption says.
 */
export function PhotoTile({
  photo,
  selecting,
  selected,
  favorite,
  onToggleSelected,
  onToggleFavorite,
  action,
}: {
  photo: Item;
  selecting?: boolean;
  selected?: boolean;
  favorite?: boolean;
  onToggleSelected?: (photo: Item) => void;
  onToggleFavorite?: (photo: Item) => void;
  /** An extra control, such as "remove from this album". */
  action?: { label: string; onAction: (photo: Item) => void };
}) {
  const picture = (
    <>
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
    </>
  );

  return (
    <li className={styles.cell}>
      {selecting ? (
        <button
          type="button"
          className={styles.tile}
          aria-pressed={selected ? "true" : "false"}
          data-selected={selected ? "true" : undefined}
          title={details(photo)}
          onClick={() => onToggleSelected?.(photo)}
        >
          {picture}
          <span className={styles.check} aria-hidden="true">
            {selected ? "✓" : ""}
          </span>
        </button>
      ) : (
        <a
          className={styles.tile}
          href={contentUrl(photo.id)}
          target="_blank"
          rel="noreferrer"
          title={details(photo)}
        >
          {picture}
        </a>
      )}

      {onToggleFavorite && !selecting ? (
        <button
          type="button"
          className={styles.star}
          aria-pressed={favorite ? "true" : "false"}
          onClick={() => onToggleFavorite(photo)}
        >
          <span aria-hidden="true">{favorite ? "★" : "☆"}</span>
          <span className={styles.hidden}>
            {favorite ? `Remove ${photo.name} from favorites` : `Add ${photo.name} to favorites`}
          </span>
        </button>
      ) : null}

      {action && !selecting ? (
        <button
          type="button"
          className={styles.remove}
          onClick={() => action.onAction(photo)}
        >
          <span aria-hidden="true">×</span>
          <span className={styles.hidden}>
            {action.label} {photo.name}
          </span>
        </button>
      ) : null}
    </li>
  );
}
