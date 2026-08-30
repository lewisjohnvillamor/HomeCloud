"use client";

import { Suspense, useId, useState, type FormEvent } from "react";
import { useSearchParams } from "next/navigation";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/states";
import { useActiveLibrary } from "@/components/session/session-provider";
import { approvePairing } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import formStyles from "@/components/ui/form.module.css";
import styles from "./pair.module.css";

export default function PairPage() {
  return (
    // `useSearchParams` needs a boundary; the form is useful without the
    // code prefilled, so the fallback is the form itself.
    <Suspense fallback={<PairForm initialCode="" />}>
      <PairFromLink />
    </Suspense>
  );
}

function PairFromLink() {
  const code = useSearchParams().get("code") ?? "";

  // Keyed on the code: scanning a second screen's square replaces the
  // form rather than leaving the first code in a field nobody edited.
  return <PairForm key={code} initialCode={code} />;
}

/**
 * Approving a television from a device that can actually type.
 *
 * Reached by scanning the square on the screen, or by typing the code
 * underneath it. The wording is explicit about what is being granted,
 * because "allow this device" with no object is how people approve
 * things they did not mean to.
 */
function PairForm({ initialCode }: { initialCode: string }) {
  const library = useActiveLibrary();
  const codeId = useId();
  const nameId = useId();

  const [code, setCode] = useState(initialCode);
  const [name, setName] = useState("Living room");
  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const [approved, setApproved] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!library) {
      return;
    }

    setProblem(null);
    setSubmitting(true);

    const result = await approvePairing(code.trim(), { library: library.id, name: name.trim() });

    if (result.ok) {
      setApproved(result.data.name);
    } else {
      setProblem(result.problem);
    }

    setSubmitting(false);
  }

  if (!library) {
    return (
      <EmptyState
        title="No library on this account"
        description="A television is paired with a library, and this account is not a member of one."
      />
    );
  }

  if (approved) {
    return (
      <>
        <h1>Connected</h1>
        <p className={styles.detail}>
          <strong>{approved}</strong> can now show photos from{" "}
          <strong>{library.name}</strong>. The screen picks it up within a few
          seconds. You can disconnect it at any time from More.
        </p>
      </>
    );
  }

  return (
    <>
      <h1>Connect a television</h1>
      <p className={styles.detail}>
        Enter the code shown on the screen. It will be able to show photos and
        videos from <strong>{library.name}</strong> — and nothing else: no
        files, no search, and no way to change anything.
      </p>

      <form className={styles.form} onSubmit={submit}>
        <div className={formStyles.field}>
          <label className={formStyles.label} htmlFor={codeId}>
            Code on the screen
          </label>
          <input
            id={codeId}
            className={formStyles.input}
            value={code}
            onChange={(event) => setCode(event.target.value)}
            autoComplete="off"
            spellCheck={false}
            autoCapitalize="characters"
            required
          />
          <span className={formStyles.hint}>Spaces and capitals do not matter.</span>
        </div>

        <div className={formStyles.field}>
          <label className={formStyles.label} htmlFor={nameId}>
            Name this screen
          </label>
          <input
            id={nameId}
            className={formStyles.input}
            value={name}
            onChange={(event) => setName(event.target.value)}
            maxLength={64}
          />
        </div>

        {problem ? (
          <p className={`${formStyles.hint} ${formStyles.error}`} role="alert">
            {problem.code === "not_found"
              ? "That code is not one we are waiting for. Codes expire after a few minutes — check the screen for a new one."
              : problem.detail}
          </p>
        ) : null}

        <div className={formStyles.actions}>
          <Button type="submit" variant="primary" disabled={submitting}>
            {submitting ? "Connecting…" : "Connect this screen"}
          </Button>
        </div>
      </form>
    </>
  );
}
