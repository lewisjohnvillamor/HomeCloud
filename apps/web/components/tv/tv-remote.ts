"use client";

/**
 * The remote-control model, in one place.
 *
 * A remote has four directions, a select button, and a back button. Any
 * TV surface that maps keys must map the same ones, or the interface
 * stops being predictable from across the room.
 */
export type RemoteAction = "left" | "right" | "up" | "down" | "select" | "back" | "playPause";

/** Translates a key press into a remote action, or `null` to ignore it. */
export function remoteAction(event: KeyboardEvent): RemoteAction | null {
  switch (event.key) {
    case "ArrowLeft":
      return "left";
    case "ArrowRight":
      return "right";
    case "ArrowUp":
      return "up";
    case "ArrowDown":
      return "down";
    case "Enter":
      return "select";
    case "Escape":
    case "Backspace":
    // Some remotes send the browser's back button.
    case "GoBack":
      return "back";
    case " ":
    case "MediaPlayPause":
      return "playPause";
    default:
      return null;
  }
}

/**
 * Moves a selection within a grid of `total` tiles laid out in `columns`.
 *
 * Clamped rather than wrapping: on a wall of photos, wrapping from the
 * end back to the beginning is disorienting when you cannot see the
 * cursor jump.
 */
export function moveSelection(
  current: number,
  action: RemoteAction,
  total: number,
  columns: number,
): number {
  if (total === 0) {
    return 0;
  }

  const step = (next: number) => Math.max(0, Math.min(total - 1, next));

  switch (action) {
    case "left":
      return step(current - 1);
    case "right":
      return step(current + 1);
    case "up":
      return step(current - columns);
    case "down":
      return step(current + columns);
    default:
      return current;
  }
}
