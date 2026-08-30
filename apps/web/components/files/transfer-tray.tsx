"use client";

import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { formatBytes } from "@/lib/format";
import styles from "./transfer-tray.module.css";

/** One file on its way to the server. */
export type Transfer = {
  id: string;
  name: string;
  sizeBytes: number;
  sent: number;
  status: "waiting" | "sending" | "done" | "failed";
  /**
   * What the file was actually called once it landed. Different from
   * `name` when something already had that name — which is the moment a
   * person needs telling, not a detail to keep quiet.
   */
  landedAs?: string;
  detail?: string;
};

/**
 * What is being sent, and what happened to it.
 *
 * A single "busy" label answers "is something happening" and nothing
 * else. Sending twenty photos raises three questions it cannot answer:
 * which one is it on, did the earlier ones work, and what happened to
 * the one that failed. The tray answers all three and stays until it is
 * dismissed, so a failure cannot vanish while somebody is looking away.
 */
export function TransferTray({
  transfers,
  onDismiss,
}: {
  transfers: Transfer[];
  onDismiss: () => void;
}) {
  if (transfers.length === 0) {
    return null;
  }

  // Still moving, rather than "not done": a failure is finished too, and
  // counting it as active left the tray saying "Sending…" forever with
  // no way to dismiss it.
  const active = transfers.filter(
    (transfer) => transfer.status === "waiting" || transfer.status === "sending",
  ).length;
  const failed = transfers.filter((transfer) => transfer.status === "failed").length;
  const finished = active === 0;
  const settled = transfers.length - active;

  return (
    <section
      className={styles.tray}
      aria-label="Transfers"
      // Announced while work is in progress, silent once it is done, so
      // a screen reader is told what is happening without repeating the
      // whole list every time a byte moves.
      aria-live="polite"
      aria-busy={!finished}
    >
      <header className={styles.header}>
        {/* Assertive only when there is bad news and the work has
            stopped: a failure among twenty files must not be a polite
            aside, and progress must not interrupt every few bytes. */}
        <h2 className={styles.title} role={finished && failed > 0 ? "alert" : undefined}>
          {finished
            ? failed > 0
              ? `${failed} of ${transfers.length} did not send`
              : `${transfers.length} sent`
            : `Sending ${Math.min(settled + 1, transfers.length)} of ${transfers.length}`}
        </h2>
        {finished ? (
          <Button variant="quiet" onClick={onDismiss}>
            <Icon name="close" />
            Dismiss
          </Button>
        ) : null}
      </header>

      <ul className={styles.list}>
        {transfers.map((transfer) => {
          const percent =
            transfer.sizeBytes > 0
              ? Math.min(100, Math.floor((transfer.sent / transfer.sizeBytes) * 100))
              : 0;

          return (
            <li key={transfer.id} className={styles.row} data-status={transfer.status}>
              <span className={styles.name}>
                {transfer.name}
                {/* The rename is the point: a person who is not told
                    ends up with two files and no idea why. */}
                {transfer.landedAs && transfer.landedAs !== transfer.name ? (
                  <span className={styles.renamed}>
                    {" "}
                    — something was already called that, so it was saved as{" "}
                    <strong>{transfer.landedAs}</strong>
                  </span>
                ) : null}
                {transfer.detail ? (
                  <span className={styles.detail}> — {transfer.detail}</span>
                ) : null}
              </span>

              <span className={styles.state}>
                {transfer.status === "sending" ? (
                  <>
                    <progress className={styles.progress} value={percent} max={100} />
                    <span className={styles.percent}>{percent}%</span>
                  </>
                ) : (
                  <span className={styles.status}>
                    {transfer.status === "done" ? (
                      <>
                        <Icon name="check" /> {formatBytes(transfer.sizeBytes)}
                      </>
                    ) : transfer.status === "failed" ? (
                      "Did not send"
                    ) : (
                      "Waiting"
                    )}
                  </span>
                )}
              </span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
