"use client";

import { use, useCallback, useId, useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import {
  fetchPublicShare,
  publicContentUrl,
  publicThumbnailUrl,
  unlockShare,
} from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { Item } from "@/lib/api/types";
import type { PublicShare } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { formatBytes, formatDate } from "@/lib/format";
import styles from "./share.module.css";

/**
 * A shared file or folder, seen by someone who is not signed in.
 *
 * Everything here comes from the link: there is no session, no library
 * navigation, and no way to reach anything the link does not cover.
 */
export default function SharePage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = use(params);
  const [openItem, setOpenItem] = useState<string | undefined>(undefined);

  // Held in memory only: a key in the URL or in storage would outlive
  // the visit and travel onwards with a copied address.
  const [key, setKey] = useState<string | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchPublicShare(token, openItem, key, { signal }),
    [token, openItem, key],
  );
  const { state, reload } = useAsyncData<PublicShare>(load);

  if (state.phase === "loading") {
    return <PendingState label="Opening the shared link…" />;
  }

  if (state.phase === "failed" && state.problem.code === "password_required") {
    // Nothing about the item has been disclosed yet — not even its name.
    return <UnlockForm token={token} onUnlocked={setKey} />;
  }

  if (state.phase === "failed") {
    // Unknown, expired, and revoked all look the same on purpose: a
    // visitor should not be able to learn that a link once existed.
    return state.problem.code === "not_found" ? (
      <EmptyState
        title="This link is not available"
        description="It may have expired, been revoked, or never existed. Ask whoever sent it for a new one."
      />
    ) : (
      <ErrorState
        title="The link could not be opened"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  const { album, item, items, relativePath } = state.data;

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <p className={styles.brand}>Shared from HomeCloud</p>
        <h1 className={styles.title}>{album ? album.name : item.name}</h1>
        <p className={styles.meta}>
          {album
            ? `Album · ${album.itemCount} photo${album.itemCount === 1 ? "" : "s"}`
            : item.kind === "folder"
              ? `Folder · ${items.length} item${items.length === 1 ? "" : "s"}`
              : `${formatBytes(item.sizeBytes)} · ${formatDate(item.modifiedAt)}`}
        </p>
      </header>

      {openItem ? (
        <button type="button" className={styles.back} onClick={() => setOpenItem(undefined)}>
          <Icon name="back" /> Back to the shared folder
        </button>
      ) : null}

      {relativePath ? <p className={styles.meta}>{relativePath}</p> : null}

      {album || item.kind !== "file" ? (
        // An album shows its pictures in the order they were arranged.
        // The listing already knows how to open one from a share.
        <FolderListing
          token={token}
          items={items}
          unlockKey={key}
          onOpen={(child) => setOpenItem(child.id)}
        />
      ) : (
        <FilePreview token={token} item={item} openItem={openItem} unlockKey={key} />
      )}
    </div>
  );
}

/**
 * The gate on a protected link. Deliberately says only that a password
 * is needed: the item’s name is part of what the password protects.
 */
function UnlockForm({
  token,
  onUnlocked,
}: {
  token: string;
  onUnlocked: (key: string) => void;
}) {
  const passwordId = useId();
  const [password, setPassword] = useState("");
  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setProblem(null);
    setSubmitting(true);

    const result = await unlockShare(token, password);

    if (result.ok) {
      onUnlocked(result.data.key);
      return;
    }

    setProblem(result.problem);
    setSubmitting(false);
  }

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <p className={styles.brand}>Shared from HomeCloud</p>
        <h1 className={styles.title}>This link is password protected</h1>
        <p className={styles.meta}>
          Whoever sent it should have given you the password separately.
        </p>
      </header>

      <form className={styles.unlock} onSubmit={submit}>
        <label className={styles.unlockLabel} htmlFor={passwordId}>
          Password
        </label>
        <input
          id={passwordId}
          className={styles.unlockInput}
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          autoComplete="off"
          autoFocus
          required
        />
        {problem ? (
          <p className={styles.unlockError} role="alert">
            {problem.detail}
          </p>
        ) : null}
        <Button type="submit" variant="primary" disabled={submitting}>
          {submitting ? "Checking…" : "Open link"}
        </Button>
      </form>
    </div>
  );
}

function FilePreview({
  token,
  item,
  openItem,
  unlockKey,
}: {
  token: string;
  item: Item;
  openItem?: string;
  unlockKey: string | null;
}) {
  return (
    <>
      {item.isImage ? (
        // The optimizer cannot reach a capability-protected origin.
        // eslint-disable-next-line @next/next/no-img-element
        <img
          className={styles.preview}
          src={publicContentUrl(token, openItem, unlockKey)}
          alt={item.name}
          decoding="async"
        />
      ) : null}

      <a
        className={styles.download}
        href={publicContentUrl(token, openItem, unlockKey)}
        download={item.name}
      >
        Download {item.name}
      </a>
    </>
  );
}

function FolderListing({
  token,
  items,
  unlockKey,
  onOpen,
}: {
  token: string;
  items: Item[];
  unlockKey: string | null;
  onOpen: (item: Item) => void;
}) {
  if (items.length === 0) {
    return <EmptyState title="This folder is empty" description="There is nothing to download." />;
  }

  return (
    <ul className={styles.list}>
      {items.map((child) => (
        <li key={child.id} className={styles.row}>
          <span className={styles.rowName}>
            {child.isImage ? (
              // As above: a capability-protected origin.
              // eslint-disable-next-line @next/next/no-img-element
              <img
                className={styles.thumb}
                src={publicThumbnailUrl(token, child.id, unlockKey)}
                alt=""
                loading="lazy"
                decoding="async"
              />
            ) : (
              <span className={styles.icon} aria-hidden="true">
                <Icon name={child.kind === "folder" ? "folder" : "file"} />
              </span>
            )}
            {child.name}
          </span>

          <span className={styles.rowActions}>
            <span className={styles.rowMeta}>
              {child.kind === "folder" ? "Folder" : formatBytes(child.sizeBytes)}
            </span>
            {child.kind === "folder" ? (
              <button type="button" className={styles.action} onClick={() => onOpen(child)}>
                Open<span className={styles.hidden}> {child.name}</span>
              </button>
            ) : (
              <a
                className={styles.action}
                href={publicContentUrl(token, child.id, unlockKey)}
                download={child.name}
              >
                Download<span className={styles.hidden}> {child.name}</span>
              </a>
            )}
          </span>
        </li>
      ))}
    </ul>
  );
}
