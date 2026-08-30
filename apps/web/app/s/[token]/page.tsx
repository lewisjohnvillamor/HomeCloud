"use client";

import { use, useCallback, useState } from "react";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import { fetchPublicShare, publicContentUrl, publicThumbnailUrl } from "@/lib/api/endpoints";
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

  const load = useCallback(
    (signal: AbortSignal) => fetchPublicShare(token, openItem, { signal }),
    [token, openItem],
  );
  const { state, reload } = useAsyncData<PublicShare>(load);

  if (state.phase === "loading") {
    return <PendingState label="Opening the shared link…" />;
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

  const { item, items, relativePath } = state.data;

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <p className={styles.brand}>Shared from HomeCloud</p>
        <h1 className={styles.title}>{item.name}</h1>
        <p className={styles.meta}>
          {item.kind === "folder"
            ? `Folder · ${items.length} item${items.length === 1 ? "" : "s"}`
            : `${formatBytes(item.sizeBytes)} · ${formatDate(item.modifiedAt)}`}
        </p>
      </header>

      {openItem ? (
        <button type="button" className={styles.back} onClick={() => setOpenItem(undefined)}>
          ← Back to the shared folder
        </button>
      ) : null}

      {relativePath ? <p className={styles.meta}>{relativePath}</p> : null}

      {item.kind === "file" ? (
        <FilePreview token={token} item={item} openItem={openItem} />
      ) : (
        <FolderListing
          token={token}
          items={items}
          onOpen={(child) => setOpenItem(child.id)}
        />
      )}
    </div>
  );
}

function FilePreview({
  token,
  item,
  openItem,
}: {
  token: string;
  item: Item;
  openItem?: string;
}) {
  return (
    <>
      {item.isImage ? (
        // The optimizer cannot reach a capability-protected origin.
        // eslint-disable-next-line @next/next/no-img-element
        <img
          className={styles.preview}
          src={publicContentUrl(token, openItem)}
          alt={item.name}
          decoding="async"
        />
      ) : null}

      <a className={styles.download} href={publicContentUrl(token, openItem)} download={item.name}>
        Download {item.name}
      </a>
    </>
  );
}

function FolderListing({
  token,
  items,
  onOpen,
}: {
  token: string;
  items: Item[];
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
                src={publicThumbnailUrl(token, child.id)}
                alt=""
                loading="lazy"
                decoding="async"
              />
            ) : (
              <span className={styles.icon} aria-hidden="true">
                {child.kind === "folder" ? "📁" : "📄"}
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
                href={publicContentUrl(token, child.id)}
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
