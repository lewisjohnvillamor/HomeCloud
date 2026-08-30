"use client";

import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button";
import { ErrorState, PendingState } from "@/components/ui/states";
import {
  fetchPasskeys,
  finishPasskeyRegistration,
  removePasskey,
  startPasskeyRegistration,
} from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { RegisteredPasskey } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { createPasskey, isPasskeySupported } from "@/lib/webauthn";
import { formatDate } from "@/lib/format";
import styles from "@/components/people/people-section.module.css";

/**
 * Passkeys for the signed-in account.
 *
 * A passkey is an addition, not a replacement: the password still works,
 * so losing a device does not lock anyone out of their own server.
 */
export function PasskeySection() {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback((signal: AbortSignal) => fetchPasskeys({ signal }), []);
  const { state, reload } = useAsyncData<RegisteredPasskey[]>(load);

  async function onAdd() {
    setBusy(true);
    setProblem(null);
    setNotice(null);

    const challenge = await startPasskeyRegistration();
    if (!challenge.ok) {
      setProblem(challenge.problem);
      setBusy(false);
      return;
    }

    try {
      const credential = await createPasskey(challenge.data.options);
      const nickname = deviceName();
      const registered = await finishPasskeyRegistration(
        challenge.data.ceremonyId,
        nickname,
        credential,
      );

      if (registered.ok) {
        setNotice(`Passkey “${nickname}” added.`);
        await reload();
      } else {
        setProblem(registered.problem);
      }
    } catch {
      // The person cancelled the browser prompt, or the authenticator
      // refused. Neither is a server error worth alarming them about.
      setNotice("No passkey was added.");
    }

    setBusy(false);
  }

  async function onRemove(passkey: RegisteredPasskey) {
    const confirmed = window.confirm(
      `Remove “${passkey.nickname}”? You can still sign in with your password.`,
    );
    if (!confirmed) {
      return;
    }

    setBusy(true);
    const result = await removePasskey(passkey.id);

    if (result.ok) {
      setNotice(`Passkey “${passkey.nickname}” removed.`);
      await reload();
    } else {
      setProblem(result.problem);
    }

    setBusy(false);
  }

  if (!isPasskeySupported()) {
    return (
      <p className={styles.meta}>
        This browser cannot use passkeys. Your password still works.
      </p>
    );
  }

  if (state.phase === "loading") {
    return <PendingState label="Loading passkeys…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="Passkeys could not be loaded"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  const passkeys = state.data;

  return (
    <>
      {passkeys.length > 0 ? (
        <ul className={styles.list}>
          {passkeys.map((passkey) => (
            <li key={passkey.id} className={styles.row}>
              <span>
                <span className={styles.name}>{passkey.nickname}</span>
                <span className={styles.meta}>
                  {" "}
                  added {formatDate(passkey.createdAt)}
                  {passkey.lastUsedAt ? ` · last used ${formatDate(passkey.lastUsedAt)}` : ""}
                </span>
              </span>
              <Button variant="quiet" onClick={() => void onRemove(passkey)} disabled={busy}>
                Remove<span className={styles.hidden}> {passkey.nickname}</span>
              </Button>
            </li>
          ))}
        </ul>
      ) : (
        <p className={styles.meta}>
          No passkeys yet. A passkey signs you in with your device instead of a password.
        </p>
      )}

      <div className={styles.actions}>
        <Button onClick={() => void onAdd()} disabled={busy}>
          Add a passkey
        </Button>
      </div>

      {notice ? (
        <p className={styles.meta} role="status">
          {notice}
        </p>
      ) : null}
      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}
    </>
  );
}

/** A recognisable default name, so a list of passkeys is readable later. */
function deviceName(): string {
  const agent = navigator.userAgent;

  if (/iPhone|iPad/.test(agent)) return "iPhone or iPad";
  if (/Android/.test(agent)) return "Android device";
  if (/Macintosh/.test(agent)) return "Mac";
  if (/Windows/.test(agent)) return "Windows PC";
  if (/Linux/.test(agent)) return "Linux computer";

  return "This device";
}
