import type { ReactNode } from "react";
import styles from "./states.module.css";

/**
 * An honest empty state: it says what is missing and why, and never
 * invents placeholder content to fill the screen.
 */
export function EmptyState({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children?: ReactNode;
}) {
  return (
    <section className={styles.state}>
      <h2 className={styles.title}>{title}</h2>
      <p className={styles.description}>{description}</p>
      {children}
    </section>
  );
}

/**
 * An error state always offers the recovery action when one exists —
 * a dead end is a bug, not a design.
 */
export function ErrorState({
  title,
  description,
  actionLabel,
  onAction,
}: {
  title: string;
  description: string;
  actionLabel?: string;
  onAction?: () => void;
}) {
  return (
    <section className={`${styles.state} ${styles.error}`} role="alert">
      <h2 className={styles.title}>{title}</h2>
      <p className={styles.description}>{description}</p>
      {actionLabel && onAction ? (
        <button type="button" className={styles.action} onClick={onAction}>
          {actionLabel}
        </button>
      ) : null}
    </section>
  );
}

/**
 * Pending state. `aria-live="polite"` announces the result to a screen
 * reader once it arrives, rather than spinning silently.
 */
export function PendingState({ label }: { label: string }) {
  return (
    <p className={styles.pending} role="status" aria-live="polite">
      {label}
    </p>
  );
}
