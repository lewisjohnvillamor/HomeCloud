"use client";

import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button";
import { ErrorState, PendingState } from "@/components/ui/states";
import {
  createInvitation,
  fetchInvitations,
  fetchMembers,
  removeMember,
  revokeInvitation,
} from "@/lib/api/endpoints";
import type { ApiResult } from "@/lib/api/client";
import type { ApiProblem } from "@/lib/api/problem";
import type { Invitation, Member } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { formatDate } from "@/lib/format";
import styles from "./people-section.module.css";

/** How long a new invitation lasts, in days. */
const INVITATION_DAYS = 7;

type People = { members: Member[]; invitations: Invitation[] };

/**
 * Who can reach this library.
 *
 * Members see the list — sharing a library with someone you cannot name
 * is not sharing — but only the owner sees the invite and remove
 * controls, and the server refuses them for anyone else regardless.
 */
export function PeopleSection({ library, isOwner }: { library: string; isOwner: boolean }) {
  const [link, setLink] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    async (signal: AbortSignal): Promise<ApiResult<People>> => {
      const members = await fetchMembers(library, { signal });
      if (!members.ok) {
        return members;
      }

      // Only the owner may list invitations; for a member the section
      // simply shows the people, with no pending list.
      if (!isOwner) {
        return { ok: true, data: { members: members.data, invitations: [] } };
      }

      const invitations = await fetchInvitations(library, { signal });
      if (!invitations.ok) {
        return invitations;
      }

      return { ok: true, data: { members: members.data, invitations: invitations.data } };
    },
    [library, isOwner],
  );

  const { state, reload } = useAsyncData<People>(load);

  async function onInvite() {
    setBusy(true);
    setProblem(null);
    setCopied(false);

    const result = await createInvitation(library, INVITATION_DAYS);

    if (result.ok && result.data.token) {
      setLink(`${window.location.origin}/invite/${result.data.token}`);
      await reload();
    } else if (!result.ok) {
      setProblem(result.problem);
    }

    setBusy(false);
  }

  async function onRevoke(invitation: Invitation) {
    setBusy(true);
    const result = await revokeInvitation(invitation.id);

    if (!result.ok) {
      setProblem(result.problem);
    }

    setLink(null);
    await reload();
    setBusy(false);
  }

  async function onRemove(member: Member) {
    const confirmed = window.confirm(
      `Remove ${member.displayName}? They lose access immediately, and any files they added stay in the library.`,
    );
    if (!confirmed) {
      return;
    }

    setBusy(true);
    const result = await removeMember(library, member.userId);

    if (!result.ok) {
      setProblem(result.problem);
    }

    await reload();
    setBusy(false);
  }

  async function onCopy() {
    if (!link) {
      return;
    }

    try {
      await navigator.clipboard.writeText(link);
      setCopied(true);
    } catch {
      // The link is on screen and selectable; a refused clipboard is not
      // worth an error message.
      setCopied(false);
    }
  }

  if (state.phase === "loading") {
    return <PendingState label="Loading people…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="People could not be loaded"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  const { members, invitations } = state.data;

  return (
    <>
      <ul className={styles.list}>
        {members.map((member) => (
          <li key={member.userId} className={styles.row}>
            <span>
              <span className={styles.name}>{member.displayName}</span>
              <span className={styles.meta}>
                {" "}
                {member.role === "owner" ? "Owner" : "Member"}
                {member.isYou ? " · you" : ""}
              </span>
            </span>
            {isOwner && member.role !== "owner" ? (
              <Button variant="quiet" onClick={() => void onRemove(member)} disabled={busy}>
                Remove<span className={styles.hidden}> {member.displayName}</span>
              </Button>
            ) : null}
          </li>
        ))}
      </ul>

      {isOwner ? (
        <>
          <div className={styles.actions}>
            <Button onClick={() => void onInvite()} disabled={busy}>
              Invite someone
            </Button>
          </div>

          {link ? (
            <>
              <p className={styles.meta}>
                Send this link to the person you are inviting. It works once, expires in{" "}
                {INVITATION_DAYS} days, and is not shown again.
              </p>
              <div className={styles.linkRow}>
                <input
                  className={styles.link}
                  readOnly
                  value={link}
                  aria-label="Invitation link"
                  onFocus={(event) => event.target.select()}
                />
                <Button onClick={() => void onCopy()}>{copied ? "Copied" : "Copy"}</Button>
              </div>
            </>
          ) : null}

          {invitations.length > 0 ? (
            <ul className={styles.list}>
              {invitations.map((invitation) => (
                <li key={invitation.id} className={styles.row}>
                  <span className={styles.meta}>
                    Invitation sent {formatDate(invitation.createdAt)} · expires{" "}
                    {formatDate(invitation.expiresAt)} · not used yet
                  </span>
                  <Button variant="quiet" onClick={() => void onRevoke(invitation)} disabled={busy}>
                    Revoke
                  </Button>
                </li>
              ))}
            </ul>
          ) : null}
        </>
      ) : null}

      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}
    </>
  );
}
