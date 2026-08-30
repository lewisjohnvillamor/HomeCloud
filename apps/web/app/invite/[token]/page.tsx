"use client";

import { use, useCallback, useId, useState, type FormEvent } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import { acceptInvitation, previewInvitation } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { InvitationPreview } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import formStyles from "@/components/ui/form.module.css";
import styles from "./invite.module.css";

/** Mirrors the server's policy so the message arrives before a round trip. */
const MIN_PASSWORD_LENGTH = 12;

/**
 * Accepting an invitation.
 *
 * Deliberately says only what the invitation covers — a library name and
 * who sent it — because that is all the server will tell someone who is
 * not yet a member.
 */
export default function InvitePage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = use(params);
  const router = useRouter();
  const nameId = useId();
  const passwordId = useId();

  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => previewInvitation(token, { signal }),
    [token],
  );
  const { state } = useAsyncData<InvitationPreview>(load);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setProblem(null);
    setSubmitting(true);

    const result = await acceptInvitation(token, { displayName, password });

    if (result.ok) {
      // Straight into the library they were invited to.
      router.push("/");
      return;
    }

    setProblem(result.problem);
    setSubmitting(false);
  }

  if (state.phase === "loading") {
    return <PendingState label="Checking the invitation…" />;
  }

  if (state.phase === "failed") {
    return state.problem.code === "not_found" ? (
      <EmptyState
        title="This invitation is not available"
        description="It may have expired, been used already, or been withdrawn. Ask whoever invited you for a new one."
      />
    ) : (
      <ErrorState title="The invitation could not be opened" description={state.problem.detail} />
    );
  }

  const invitation = state.data;

  return (
    <div className={styles.screen}>
      <div className={styles.lockup}>
        <p className={styles.name}>HomeCloud</p>
        <p className={styles.promise}>
          <strong>{invitation.invitedBy}</strong> invited you to{" "}
          <strong>{invitation.libraryName}</strong>.
        </p>
      </div>

      <form className={formStyles.form} onSubmit={submit}>
        <h2 className={formStyles.title}>Create your account</h2>
        <p className={formStyles.hint}>
          Your account lives on this server, alongside the files. Nothing is sent anywhere
          else.
        </p>

        <div className={formStyles.field}>
          <label className={formStyles.label} htmlFor={nameId}>
            Your name
          </label>
          <input
            id={nameId}
            className={formStyles.input}
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            autoComplete="username"
            required
            maxLength={64}
          />
        </div>

        <div className={formStyles.field}>
          <label className={formStyles.label} htmlFor={passwordId}>
            Password
          </label>
          <input
            id={passwordId}
            className={formStyles.input}
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            autoComplete="new-password"
            required
            minLength={MIN_PASSWORD_LENGTH}
          />
          <span className={formStyles.hint}>
            At least {MIN_PASSWORD_LENGTH} characters. A memorable phrase works well.
          </span>
        </div>

        {problem ? (
          <p className={`${formStyles.hint} ${formStyles.error}`} role="alert">
            {problem.detail}
          </p>
        ) : null}

        <div className={formStyles.actions}>
          <Button type="submit" variant="primary" disabled={submitting}>
            {submitting ? "Joining…" : "Join the library"}
          </Button>
        </div>
      </form>
    </div>
  );
}
