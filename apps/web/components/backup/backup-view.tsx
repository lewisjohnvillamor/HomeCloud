"use client";

import { useCallback, useRef, useState } from "react";
import { TransferTray, type Transfer } from "@/components/files/transfer-tray";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { ErrorState, PendingState } from "@/components/ui/states";
import {
  BACKUP_BATCH,
  checkBackup,
  fetchBackupDevices,
  finishBackup,
  registerBackupDevice,
} from "@/lib/api/endpoints";
import { sendFile } from "@/lib/api/send-file";
import type { ApiProblem } from "@/lib/api/problem";
import type { BackupDevice } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import styles from "./backup-view.module.css";

/**
 * Backing up a phone's photographs.
 *
 * The honest shape of this on the web: a browser cannot read a camera
 * roll on its own, so nothing here happens while the page is closed.
 * What it can do is make the repeat cheap — select everything, and only
 * what is new actually goes up. The page says that plainly rather than
 * implying a background service it does not have.
 */
export function BackupView({ library }: { library: string }) {
  const load = useCallback(
    (signal: AbortSignal) => fetchBackupDevices(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<BackupDevice[]>(load);

  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const [transfers, setTransfers] = useState<Transfer[]>([]);
  const [summary, setSummary] = useState<string | null>(null);
  const chooser = useRef<HTMLInputElement>(null);

  if (state.phase === "loading") {
    return <PendingState label="Loading your devices…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="Your devices could not be loaded"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  const device = state.data[0] ?? null;

  async function onRegister(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setProblem(null);

    const result = await registerBackupDevice(library, name);
    if (!result.ok) {
      setProblem(result.problem);
      setBusy(false);
      return;
    }

    setName("");
    await reload();
    setBusy(false);
  }

  async function onChoose(event: React.ChangeEvent<HTMLInputElement>) {
    const chosen = Array.from(event.target.files ?? []);
    if (chosen.length === 0 || !device) {
      return;
    }

    setBusy(true);
    setProblem(null);
    setSummary(null);
    setTransfers([]);

    // Ask in batches. A camera roll is bigger than one request should
    // be, and asking in pieces means the answer starts arriving instead
    // of the page sitting silent through one enormous round trip.
    const wanted = new Set<string>();
    let alreadyHere = 0;

    for (let at = 0; at < chosen.length; at += BACKUP_BATCH) {
      const batch = chosen.slice(at, at + BACKUP_BATCH);
      const answer = await checkBackup(
        library,
        device.id,
        batch.map((file) => ({ name: file.name, sizeBytes: file.size })),
      );

      if (!answer.ok) {
        setProblem(answer.problem);
        setBusy(false);
        return;
      }

      alreadyHere += answer.data.alreadyHere;
      for (const missing of answer.data.missing) {
        wanted.add(missing);
      }
    }

    const sending = chosen.filter((file) => wanted.has(file.name));

    if (sending.length === 0) {
      setSummary(
        `Everything is already here — all ${chosen.length.toLocaleString()} of them. Nothing to send.`,
      );
      setBusy(false);
      resetChooser();
      return;
    }

    const queued: Transfer[] = sending.map((file, index) => ({
      id: `${Date.now()}-${index}-${file.name}`,
      name: file.name,
      sizeBytes: file.size,
      sent: 0,
      status: "waiting",
    }));
    setTransfers(queued);

    function update(id: string, change: Partial<Transfer>) {
      setTransfers((current) =>
        current.map((transfer) => (transfer.id === id ? { ...transfer, ...change } : transfer)),
      );
    }

    let sent = 0;
    for (const [index, file] of sending.entries()) {
      const id = queued[index]?.id ?? "";
      update(id, { status: "sending" });

      const result = await sendFile(
        { library, path: `${device.folder}/${file.name}`, file },
        ({ sent: moved }) => update(id, { sent: moved }),
      );

      if (result.ok) {
        sent += 1;
        update(id, { status: "done", sent: file.size, landedAs: result.data.name });
      } else {
        update(id, { status: "failed", detail: result.problem.detail });
      }
    }

    await finishBackup(library, device.id, sent);
    await reload();

    setSummary(
      alreadyHere > 0
        ? `${sent.toLocaleString()} sent. ${alreadyHere.toLocaleString()} were already here.`
        : `${sent.toLocaleString()} sent.`,
    );
    setBusy(false);
    resetChooser();
  }

  function resetChooser() {
    if (chooser.current) {
      chooser.current.value = "";
    }
  }

  return (
    <section className={styles.backup}>
      {/* Said once, at the top, because somebody who assumes this runs
          by itself will stop checking and lose photographs. */}
      <p className={styles.truth}>
        This does not run on its own. A web page cannot read your camera roll while it is
        closed, so backing up means opening this page and choosing your photos. Choosing all
        of them is fine — anything already here is skipped.
      </p>

      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}

      {device ? (
        <>
          <div className={styles.device}>
            <h2 className={styles.deviceName}>{device.name}</h2>
            <dl className={styles.facts}>
              <div className={styles.fact}>
                <dt>Photos here</dt>
                <dd>{device.photoCount.toLocaleString()}</dd>
              </div>
              <div className={styles.fact}>
                <dt>Last backup</dt>
                <dd>{device.lastBackupAt ? whenever(device.lastBackupAt) : "Never"}</dd>
              </div>
              <div className={styles.fact}>
                <dt>Saved in</dt>
                <dd className={styles.folder}>{device.folder}</dd>
              </div>
            </dl>
          </div>

          <label className={styles.choose}>
            <input
              ref={chooser}
              type="file"
              multiple
              accept="image/*,video/*"
              className={styles.file}
              disabled={busy}
              aria-label="Choose photos to back up"
              onChange={(event) => void onChoose(event)}
            />
            {/* The input itself is off-screen; this is what a person
                sees and taps, so it must not also claim the name. */}
            <span className={styles.chooseLabel} aria-hidden="true">
              <Icon name="upload" />
              {busy ? "Working…" : "Choose photos to back up"}
            </span>
          </label>

          {summary ? (
            <p className={styles.summary} role="status">
              {summary}
            </p>
          ) : null}

          <TransferTray transfers={transfers} onDismiss={() => setTransfers([])} />
        </>
      ) : (
        <form className={styles.setup} onSubmit={(event) => void onRegister(event)}>
          <label className={styles.field}>
            <span>What is this phone called?</span>
            <input
              className={styles.input}
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Ada's phone"
              maxLength={60}
              required
            />
          </label>
          <p className={styles.hint}>
            Its photographs go in a folder of this name, so you can find them on the disk
            without this app.
          </p>
          <Button type="submit" variant="primary" disabled={busy || name.trim() === ""}>
            Set up backup
          </Button>
        </form>
      )}
    </section>
  );
}

/**
 * A date somebody can read at a glance. "Never" is handled by the
 * caller, because the absence of a backup is not a date at all.
 */
function whenever(iso: string): string {
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) {
    return "Unknown";
  }

  const minutes = Math.round((Date.now() - at.getTime()) / 60_000);
  if (minutes < 1) {
    return "Just now";
  }
  if (minutes < 60) {
    return `${minutes} minute${minutes === 1 ? "" : "s"} ago`;
  }

  const hours = Math.round(minutes / 60);
  if (hours < 24) {
    return `${hours} hour${hours === 1 ? "" : "s"} ago`;
  }

  return at.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}
