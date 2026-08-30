# Multi-User Implementation Plan

> Completes the membership half of Phase 3 in `ROADMAP.md`. Sharing landed in
> `2026-08-29-catalog-transfers-and-ui.md`; this plan lets a second person have
> an account rather than a link.

**Goal:** The owner can invite someone to a library, that person creates their
own account from the invitation, and from then on the server decides what each
of them can do — with the owner keeping powers a member does not have.

**Architecture:** An `invitations` table using the same bearer-token rules as
sessions and shares, role checks in one place, and a members list the owner
manages. The domain already models the rules (`LibraryRole`, "the owner cannot
be removed"); this plan enforces them at the database and the API.

## Tasks

1. **Invitations** — create, list, revoke. Owner-only. Bounded expiry. Token
   shown once, stored hashed.
2. **Acceptance** — a public endpoint that says what the invitation is for
   (library name and who sent it, nothing else), and one that accepts it by
   creating an account and a session, or by adding the signed-in user.
3. **Roles** — one place that answers "may this user administer this library".
   Members read and write content; only the owner manages membership.
4. **Members** — list, and remove. Removal takes effect immediately; the owner
   cannot be removed, enforced by the database as well as the domain.
5. **UI** — a People section in More for the owner, and an invitation page for
   the person accepting.
6. **Adversarial tests** — a member cannot invite, cannot remove anyone, and
   cannot escalate to owner; a removed member loses access at once; an expired
   or revoked invitation is indistinguishable from one that never existed.

## Self-Review

- Passkeys and account recovery stay out of scope; both deserve their own plan,
  and an invitation flow does not depend on either.
- Per-folder permissions are deliberately absent: library membership is the
  authorization boundary the whole system already assumes, and narrowing it
  belongs with a design that covers shares and sync too.
