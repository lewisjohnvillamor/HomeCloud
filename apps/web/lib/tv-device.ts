"use client";

import { useSyncExternalStore } from "react";

/**
 * Where a paired television keeps its credential.
 *
 * `localStorage` rather than a cookie: the token is not a session, the
 * server never sets it, and a television is a single-purpose device that
 * should stay paired across power cuts.
 *
 * It is exposed as an external store rather than as state copied into an
 * effect, because that is what it is — a value owned by the browser that
 * React reads. Reads are guarded because a browser may refuse storage
 * entirely; an unpaired screen is a recoverable state, it simply shows
 * the pairing code again.
 */
const KEY = "homecloud.tv.token";

/** Cached so repeated reads return an identical value, as React requires. */
let snapshot: string | null = null;
let loaded = false;

const listeners = new Set<() => void>();

function read(): string | null {
  try {
    return window.localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);

  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): string | null {
  if (!loaded) {
    snapshot = read();
    loaded = true;
  }

  return snapshot;
}

/**
 * The token, or `undefined` before the browser has been asked — which is
 * the server's answer too, so a paired screen does not flash a pairing
 * code while it hydrates.
 */
export function useDeviceToken(): string | null | undefined {
  return useSyncExternalStore(subscribe, getSnapshot, () => undefined);
}

export function rememberDeviceToken(token: string): void {
  try {
    window.localStorage.setItem(KEY, token);
  } catch {
    // A screen that cannot remember its token still works for this
    // session; it will ask to be paired again after a reload.
  }

  snapshot = token;
  loaded = true;
  for (const listener of listeners) {
    listener();
  }
}

export function forgetDeviceToken(): void {
  try {
    window.localStorage.removeItem(KEY);
  } catch {
    // Nothing to do: there was nothing to forget.
  }

  snapshot = null;
  loaded = true;
  for (const listener of listeners) {
    listener();
  }
}
