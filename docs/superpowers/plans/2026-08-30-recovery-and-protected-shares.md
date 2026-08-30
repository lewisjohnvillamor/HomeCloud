# Account Recovery and Password-Protected Shares

> Two named gaps: `ROADMAP.md` Phase 3 lists recovery, and
> `SECURITY_AND_PRIVACY.md` §16 records that share links have no second factor.

**Goal:** Forgetting a password on a server in your own house should not need
database surgery, and a link sent over a channel you do not fully trust should
be able to carry a password.

## Tasks

1. **Recovery codes** — a single high-entropy code per account, stored hashed,
   shown once at setup and regenerable. Using it sets a new password, ends
   every session, and burns the code.
2. **Throttling** — recovery attempts share the sign-in limiter, so a code
   cannot be guessed any faster than a password.
3. **Protected shares** — an optional password on a share link. The visitor
   proves it before anything about the item is disclosed, including its name.
4. **UI** — "Forgot your password?" on sign-in; a recovery section in More; a
   password field in the share dialog and an unlock screen on the public page.
5. **Adversarial tests** — a wrong code, a used code, another account's code, a
   wrong share password, and a share whose password is guessed at repeatedly.

## Self-Review

- A recovery code is a password-equivalent secret; it is stored with the same
  Argon2 hashing as a password, not as a plain token, because it is chosen from
  a small enough space to be worth attacking offline if the database leaks.
- Recovery deliberately does not use email: this server may have no outbound
  mail, and inventing an email dependency for the recovery path would make the
  most fragile flow depend on the most fragile infrastructure.
