"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import styles from "./recovery-code.module.css";

/**
 * Shows a recovery code once, at the only moment it exists in the clear.
 *
 * The server stores a hash, so there is no second chance to display it.
 * The screen is deliberately hard to skip past: continuing is one
 * deliberate click, and the code is selectable and copyable.
 */
export function RecoveryCodeNotice({
  code,
  continueLabel,
  onContinue,
}: {
  code: string;
  continueLabel: string;
  onContinue: () => void;
}) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
    } catch {
      // Clipboard access can be refused. The code is on screen and
      // selectable, so this is not worth interrupting for.
      setCopied(false);
    }
  }

  return (
    <div className={styles.notice}>
      <h2 className={styles.title}>Write this down</h2>
      <p className={styles.detail}>
        This recovery code is the only way back into your account if you
        forget your password. There is no email reset on a server in your
        home. It is shown once and never again.
      </p>

      <p className={styles.code} aria-label="Your recovery code">
        {code}
      </p>

      <div className={styles.actions}>
        <Button onClick={() => void copy()}>{copied ? "Copied" : "Copy code"}</Button>
        <Button variant="primary" onClick={onContinue}>
          {continueLabel}
        </Button>
      </div>
    </div>
  );
}
