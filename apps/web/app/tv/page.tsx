"use client";

import { useCallback, useMemo } from "react";
import { PairingScreen } from "@/components/tv/pairing-screen";
import { PhotoWall, pairedSource, sessionSource } from "@/components/tv/photo-wall";
import { useActiveLibrary, useSession } from "@/components/session/session-provider";
import { forgetDeviceToken, rememberDeviceToken, useDeviceToken } from "@/lib/tv-device";

/**
 * The television.
 *
 * Two ways in, in the order of what the screen already has: a credential
 * it was given when someone paired it, or an ordinary session if this is
 * a browser that happens to be signed in. A screen with neither shows a
 * pairing code, which is the case a remote control can actually get
 * through — there is no password form here, because a four-direction
 * remote cannot fill one in.
 */
export default function TvPage() {
  const { state } = useSession();
  const library = useActiveLibrary();
  const device = useDeviceToken();

  const onPaired = useCallback((token: string) => rememberDeviceToken(token), []);

  // Memoised: the wall reloads whenever its source changes identity, so
  // a fresh object on every render would poll the server forever.
  const source = useMemo(() => {
    if (device) {
      // If the screen has been disconnected, drop the credential and
      // fall back to the pairing code rather than showing an error to a
      // room with no keyboard in it.
      return pairedSource(device, forgetDeviceToken);
    }

    return state.phase === "signed-in" && library ? sessionSource(library.id) : null;
  }, [device, library, state.phase]);

  // Still asking the browser what it remembers.
  if (device === undefined) {
    return null;
  }

  return source ? <PhotoWall source={source} /> : <PairingScreen onPaired={onPaired} />;
}
