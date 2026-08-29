"use client";

import { createContext, useCallback, useContext, useMemo, type ReactNode } from "react";
import {
  fetchBootstrapStatus,
  fetchLibraries,
  fetchSession,
  signOut as signOutRequest,
} from "@/lib/api/endpoints";
import type { ApiResult } from "@/lib/api/client";
import type { ApiProblem } from "@/lib/api/problem";
import type { Library, Session } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";

export type SessionState =
  | { phase: "loading" }
  /** Reached the server; nobody is signed in. */
  | { phase: "anonymous"; needsOwner: boolean }
  | { phase: "signed-in"; session: Session; libraries: Library[] }
  | { phase: "failed"; problem: ApiProblem };

type SessionContextValue = {
  state: SessionState;
  /** Re-reads session and libraries; used after signing in or out. */
  refresh: () => Promise<void>;
  signOut: () => Promise<void>;
};

const SessionContext = createContext<SessionContextValue | undefined>(undefined);

/**
 * Holds who is signed in and which libraries they can see.
 *
 * Nothing here is an authorization decision: the server refuses requests
 * on its own. This only decides what the app can usefully render.
 */
/** What one look at the server tells us about the current visitor. */
type Snapshot =
  | { authenticated: false; needsOwner: boolean }
  | { authenticated: true; session: Session; libraries: Library[] };

async function loadSnapshot(signal: AbortSignal): Promise<ApiResult<Snapshot>> {
  const session = await fetchSession({ signal });

  if (!session.ok) {
    return session;
  }

  if (!session.data.authenticated) {
    // A failed bootstrap check is not fatal: the sign-in screen is the
    // safe thing to show when we cannot tell whether setup is done.
    const bootstrap = await fetchBootstrapStatus({ signal });

    return {
      ok: true,
      data: {
        authenticated: false,
        needsOwner: bootstrap.ok ? bootstrap.data.needsOwner : false,
      },
    };
  }

  const libraries = await fetchLibraries({ signal });

  return {
    ok: true,
    data: {
      authenticated: true,
      session: session.data,
      libraries: libraries.ok ? libraries.data : [],
    },
  };
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const { state: loaded, reload } = useAsyncData<Snapshot>(loadSnapshot);

  const state: SessionState = useMemo(() => {
    if (loaded.phase === "loading") {
      return { phase: "loading" };
    }
    if (loaded.phase === "failed") {
      return { phase: "failed", problem: loaded.problem };
    }

    return loaded.data.authenticated
      ? {
          phase: "signed-in",
          session: loaded.data.session,
          libraries: loaded.data.libraries,
        }
      : { phase: "anonymous", needsOwner: loaded.data.needsOwner };
  }, [loaded]);

  const refresh = useCallback(async () => {
    await reload();
  }, [reload]);

  const signOut = useCallback(async () => {
    await signOutRequest();
    await refresh();
  }, [refresh]);

  const value = useMemo(() => ({ state, refresh, signOut }), [state, refresh, signOut]);

  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession(): SessionContextValue {
  const value = useContext(SessionContext);

  if (!value) {
    throw new Error("useSession must be used inside a SessionProvider");
  }

  return value;
}

/**
 * The library the app is currently working in, or `null` when there is
 * none yet. A single-library deployment is the common case; this is the
 * seam where a library switcher will go.
 */
export function useActiveLibrary(): Library | null {
  const { state } = useSession();

  return state.phase === "signed-in" ? (state.libraries[0] ?? null) : null;
}
