"use client";

import { useId, useState, type FormEvent } from "react";
import { createOwner, signIn } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import { Button } from "@/components/ui/button";
import styles from "@/components/ui/form.module.css";
import { useSession } from "./session-provider";

/** Mirrors the server's policy so the message arrives before a round trip. */
const MIN_PASSWORD_LENGTH = 12;

function ProblemMessage({ problem }: { problem: ApiProblem }) {
  return (
    <p className={`${styles.hint} ${styles.error}`} role="alert">
      {problem.detail}
      {problem.requestId ? ` (reference ${problem.requestId})` : ""}
    </p>
  );
}

/**
 * First-run screen. Creates the owner account, its library, and a
 * session in one step, because a deployment with an account but no way
 * in would be a dead end.
 */
export function SetupForm() {
  const { refresh } = useSession();
  const nameId = useId();
  const passwordId = useId();
  const libraryId = useId();

  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [libraryName, setLibraryName] = useState("Home");
  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setProblem(null);
    setSubmitting(true);

    const result = await createOwner({ displayName, password, libraryName });

    if (result.ok) {
      await refresh();
      return;
    }

    setProblem(result.problem);
    setSubmitting(false);
  }

  return (
    <form className={styles.form} onSubmit={submit}>
      <h2 className={styles.title}>Set up HomeCloud</h2>
      <p className={styles.hint}>
        This creates the owner account for this server. It is stored on your own hardware.
      </p>

      <div className={styles.field}>
        <label className={styles.label} htmlFor={nameId}>
          Your name
        </label>
        <input
          id={nameId}
          className={styles.input}
          value={displayName}
          onChange={(event) => setDisplayName(event.target.value)}
          autoComplete="username"
          required
          maxLength={64}
        />
      </div>

      <div className={styles.field}>
        <label className={styles.label} htmlFor={passwordId}>
          Password
        </label>
        <input
          id={passwordId}
          className={styles.input}
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          autoComplete="new-password"
          required
          minLength={MIN_PASSWORD_LENGTH}
        />
        <span className={styles.hint}>
          At least {MIN_PASSWORD_LENGTH} characters. A memorable phrase works well.
        </span>
      </div>

      <div className={styles.field}>
        <label className={styles.label} htmlFor={libraryId}>
          Library name
        </label>
        <input
          id={libraryId}
          className={styles.input}
          value={libraryName}
          onChange={(event) => setLibraryName(event.target.value)}
          required
          maxLength={64}
        />
      </div>

      {problem ? <ProblemMessage problem={problem} /> : null}

      <div className={styles.actions}>
        <Button type="submit" variant="primary" disabled={submitting}>
          {submitting ? "Creating account…" : "Create owner account"}
        </Button>
      </div>
    </form>
  );
}

/** Sign-in screen for a deployment that already has an owner. */
export function SignInForm() {
  const { refresh } = useSession();
  const nameId = useId();
  const passwordId = useId();

  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setProblem(null);
    setSubmitting(true);

    const result = await signIn({ displayName, password });

    if (result.ok) {
      await refresh();
      return;
    }

    setProblem(result.problem);
    setSubmitting(false);
  }

  return (
    <form className={styles.form} onSubmit={submit}>
      <h2 className={styles.title}>Sign in</h2>

      <div className={styles.field}>
        <label className={styles.label} htmlFor={nameId}>
          Your name
        </label>
        <input
          id={nameId}
          className={styles.input}
          value={displayName}
          onChange={(event) => setDisplayName(event.target.value)}
          autoComplete="username"
          required
        />
      </div>

      <div className={styles.field}>
        <label className={styles.label} htmlFor={passwordId}>
          Password
        </label>
        <input
          id={passwordId}
          className={styles.input}
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          autoComplete="current-password"
          required
        />
      </div>

      {problem ? <ProblemMessage problem={problem} /> : null}

      <div className={styles.actions}>
        <Button type="submit" variant="primary" disabled={submitting}>
          {submitting ? "Signing in…" : "Sign in"}
        </Button>
      </div>
    </form>
  );
}
