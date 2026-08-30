"use client";

import { useId, useState, type FormEvent } from "react";
import {
  createOwner,
  finishPasskeySignIn,
  recoverAccount,
  signIn,
  startPasskeySignIn,
} from "@/lib/api/endpoints";
import { authenticateWithPasskey, isPasskeySupported } from "@/lib/webauthn";
import type { ApiProblem } from "@/lib/api/problem";
import { Button } from "@/components/ui/button";
import styles from "@/components/ui/form.module.css";
import { AuthScreen } from "./auth-screen";
import { RecoveryCodeNotice } from "./recovery-code";
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
  const [recoveryCode, setRecoveryCode] = useState<string | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setProblem(null);
    setSubmitting(true);

    const result = await createOwner({ displayName, password, libraryName });

    if (result.ok) {
      // The account exists and the session cookie is already set, but
      // the code is in this response only — show it before going on.
      if (result.data.recoveryCode) {
        setRecoveryCode(result.data.recoveryCode);
        return;
      }

      await refresh();
      return;
    }

    setProblem(result.problem);
    setSubmitting(false);
  }

  if (recoveryCode) {
    return (
      <AuthScreen promise="Your files and photos, on hardware you own.">
        <RecoveryCodeNotice
          code={recoveryCode}
          continueLabel="I have written it down"
          onContinue={() => void refresh()}
        />
      </AuthScreen>
    );
  }

  return (
    <AuthScreen
      promise="Your files and photos, on hardware you own."
      footnote="Nothing leaves this machine. When setup finishes, HomeCloud reads the library folder it was pointed at and shows you what is already there."
    >
      <form className={styles.form} onSubmit={submit}>
        <h2 className={styles.title}>Set up HomeCloud</h2>
        <p className={styles.hint}>
          This creates the owner account for this server.
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
            At least {MIN_PASSWORD_LENGTH} characters. A memorable phrase works
            well.
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
    </AuthScreen>
  );
}

/** Sign-in screen for a deployment that already has an owner. */
export function SignInForm() {
  const { refresh } = useSession();
  const nameId = useId();
  const passwordId = useId();
  const [recovering, setRecovering] = useState(false);

  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const [submitting, setSubmitting] = useState(false);

  /// Signing in with a device instead of a password. Offered only when
  /// the browser supports it; the password field stays either way.
  async function signInWithPasskey() {
    setProblem(null);

    if (!displayName.trim()) {
      setProblem({ code: "bad_request", detail: "Enter your name first." });
      return;
    }

    setSubmitting(true);
    const challenge = await startPasskeySignIn(displayName.trim());

    if (!challenge.ok) {
      setProblem(challenge.problem);
      setSubmitting(false);
      return;
    }

    try {
      const credential = await authenticateWithPasskey(challenge.data.options);
      const result = await finishPasskeySignIn(challenge.data.ceremonyId, credential);

      if (result.ok) {
        await refresh();
        return;
      }

      setProblem(result.problem);
    } catch {
      setProblem({ code: "bad_request", detail: "No passkey was used." });
    }

    setSubmitting(false);
  }

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

  if (recovering) {
    return <RecoverForm onCancel={() => setRecovering(false)} />;
  }

  return (
    <AuthScreen promise="Your files and photos, on hardware you own.">
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
          {isPasskeySupported() ? (
            <Button onClick={() => void signInWithPasskey()} disabled={submitting}>
              Use a passkey
            </Button>
          ) : null}
        </div>

        <Button variant="quiet" onClick={() => setRecovering(true)} disabled={submitting}>
          Forgot your password?
        </Button>
      </form>
    </AuthScreen>
  );
}

/**
 * Setting a new password from a recovery code.
 *
 * No email, because this deployment may have no way to send any: the
 * code written down at setup is the whole credential. A successful
 * recovery ends every existing session and hands back a fresh code,
 * which is shown once here before the app opens.
 */
export function RecoverForm({ onCancel }: { onCancel: () => void }) {
  const { refresh } = useSession();
  const nameId = useId();
  const codeId = useId();
  const passwordId = useId();

  const [displayName, setDisplayName] = useState("");
  const [recoveryCode, setRecoveryCode] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [nextCode, setNextCode] = useState<string | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setProblem(null);
    setSubmitting(true);

    const result = await recoverAccount({ displayName, recoveryCode, newPassword });

    if (result.ok) {
      if (result.data.recoveryCode) {
        setNextCode(result.data.recoveryCode);
        return;
      }

      await refresh();
      return;
    }

    setProblem(result.problem);
    setSubmitting(false);
  }

  if (nextCode) {
    return (
      <AuthScreen promise="Your files and photos, on hardware you own.">
        <RecoveryCodeNotice
          code={nextCode}
          continueLabel="I have written it down"
          onContinue={() => void refresh()}
        />
      </AuthScreen>
    );
  }

  return (
    <AuthScreen
      promise="Your files and photos, on hardware you own."
      footnote="Recovering signs out every device that was signed in, and replaces the code you just used with a new one."
    >
      <form className={styles.form} onSubmit={submit}>
        <h2 className={styles.title}>Use your recovery code</h2>
        <p className={styles.hint}>
          The code you wrote down when this server was set up.
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
          />
        </div>

        <div className={styles.field}>
          <label className={styles.label} htmlFor={codeId}>
            Recovery code
          </label>
          <input
            id={codeId}
            className={styles.input}
            value={recoveryCode}
            onChange={(event) => setRecoveryCode(event.target.value)}
            autoComplete="off"
            spellCheck={false}
            required
          />
          <span className={styles.hint}>
            Spaces and capitals do not matter.
          </span>
        </div>

        <div className={styles.field}>
          <label className={styles.label} htmlFor={passwordId}>
            New password
          </label>
          <input
            id={passwordId}
            className={styles.input}
            type="password"
            value={newPassword}
            onChange={(event) => setNewPassword(event.target.value)}
            autoComplete="new-password"
            required
            minLength={MIN_PASSWORD_LENGTH}
          />
          <span className={styles.hint}>At least {MIN_PASSWORD_LENGTH} characters.</span>
        </div>

        {problem ? <ProblemMessage problem={problem} /> : null}

        <div className={styles.actions}>
          <Button type="submit" variant="primary" disabled={submitting}>
            {submitting ? "Checking…" : "Set a new password"}
          </Button>
          <Button variant="quiet" onClick={onCancel} disabled={submitting}>
            Back to sign in
          </Button>
        </div>
      </form>
    </AuthScreen>
  );
}
