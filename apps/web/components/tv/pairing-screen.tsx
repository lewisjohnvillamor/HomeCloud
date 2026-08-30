"use client";

import { useEffect, useRef, useState } from "react";
import QRCode from "qrcode";
import { pollPairing, startPairing } from "@/lib/api/endpoints";
import type { Pairing } from "@/lib/api/types";
import styles from "./pairing-screen.module.css";

/** How often the television asks whether anyone has approved it. */
const POLL_MS = 2000;

/**
 * What a television shows when it is not paired yet.
 *
 * Everything is sized to be read across a room, and there is nothing to
 * type: the whole point is that a four-direction remote cannot enter a
 * password. The code is short and drawn from an alphabet without
 * look-alike characters, and the QR code is the same thing for anyone
 * holding a phone.
 */
export function PairingScreen({ onPaired }: { onPaired: (token: string) => void }) {
  const [pairing, setPairing] = useState<Pairing | null>(null);
  const [qr, setQr] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);

  // The callback is held in a ref so re-rendering the parent does not
  // restart the pairing and change the code on screen mid-approval.
  const paired = useRef(onPaired);

  useEffect(() => {
    paired.current = onPaired;
  }, [onPaired]);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    async function begin() {
      const opened = await startPairing();

      if (cancelled) {
        return;
      }
      if (!opened.ok) {
        setProblem(opened.problem.detail);
        return;
      }

      setPairing(opened.data);

      const link = `${window.location.origin}/pair?code=${encodeURIComponent(opened.data.code)}`;
      QRCode.toDataURL(link, { width: 512, margin: 1 })
        .then((image) => {
          if (!cancelled) {
            setQr(image);
          }
        })
        .catch(() => {
          // The code below it is the real credential; a missing picture
          // is a smaller screen, not a broken one.
          if (!cancelled) {
            setQr(null);
          }
        });

      const check = async () => {
        const status = await pollPairing(opened.data.pollToken);

        if (cancelled) {
          return;
        }

        if (!status.ok) {
          // The code expired, or was collected by something else.
          // Starting over is the only useful thing a television can do.
          void begin();
          return;
        }

        if (status.data.status === "approved" && status.data.token) {
          paired.current(status.data.token);
          return;
        }

        timer = setTimeout(() => void check(), POLL_MS);
      };

      timer = setTimeout(() => void check(), POLL_MS);
    }

    void begin();

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, []);

  if (problem) {
    return (
      <div className={styles.screen}>
        <h1 className={styles.title}>This screen cannot reach the server</h1>
        <p className={styles.detail} role="alert">
          {problem}
        </p>
      </div>
    );
  }

  return (
    <div className={styles.screen}>
      <h1 className={styles.title}>Connect this screen</h1>
      <p className={styles.detail}>
        On your phone, open HomeCloud and go to <strong>/pair</strong>, or scan
        the square. Then enter this code.
      </p>

      <div className={styles.pair}>
        <p className={styles.code} aria-label="Pairing code">
          {pairing ? pairing.code : "…"}
        </p>

        {qr ? (
          // A data URI generated on this device: nothing is fetched.
          // eslint-disable-next-line @next/next/no-img-element
          <img className={styles.qr} src={qr} alt="" />
        ) : (
          <div className={styles.qr} aria-hidden="true" />
        )}
      </div>

      <p className={styles.detail}>
        The code stops working after a few minutes. This screen will show photos
        only — it cannot browse your files.
      </p>
    </div>
  );
}
