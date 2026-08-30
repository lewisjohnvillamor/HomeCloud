"use client";

import { useCallback, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { RequestDialog } from "@/components/share/request-dialog";
import { ShareDialog } from "@/components/share/share-dialog";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import {
  browse as browseRequest,
  contentUrl,
  createFolder,
  moveItem,
  thumbnailUrl,
  trashItem,
} from "@/lib/api/endpoints";
import { sendFile } from "@/lib/api/send-file";
import type { ApiProblem } from "@/lib/api/problem";
import type { Browse, Item } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { formatBytes, formatDate, joinPath, parentOf } from "@/lib/format";
import styles from "./file-browser.module.css";

export type FileBrowserProps = {
  library: string;
  /** Current folder path, "" for the library root. */
  path: string;
  /** Called when the user navigates; the page owns the URL. */
  onNavigate: (path: string) => void;
};

/**
 * The file list.
 *
 * Everything here is a real button or link, so the whole view works from
 * the keyboard and reads correctly to a screen reader without any
 * bespoke focus management.
 */
export function FileBrowser({ library, path, onNavigate }: FileBrowserProps) {
  const [busy, setBusy] = useState<string | null>(null);
  const [sharing, setSharing] = useState<Item | null>(null);
  const [requesting, setRequesting] = useState<Item | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [problem, setProblem] = useState<ApiProblem | null>(null);
  const uploadInput = useRef<HTMLInputElement>(null);

  const load = useCallback(
    (signal: AbortSignal) => browseRequest(library, path, { signal }),
    [library, path],
  );
  const { state, reload } = useAsyncData<Browse>(load);

  const run = useCallback(
    async (
      label: string,
      action: () => Promise<{ ok: boolean; problem?: ApiProblem }>,
    ) => {
      setBusy(label);
      setProblem(null);
      setNotice(null);

      const result = await action();

      if (!result.ok && result.problem) {
        setProblem(result.problem);
      }

      await reload();
      setBusy(null);

      return result.ok;
    },
    [reload],
  );

  async function onUpload(files: FileList | null) {
    if (!files || files.length === 0) {
      return;
    }

    let uploaded = 0;
    for (const file of Array.from(files)) {
      const ok = await run(`Uploading ${file.name}`, async () => {
        const result = await sendFile(
          { library, path: joinPath(path, file.name), file },
          // A large file is sent in pieces, so say how far along it is
          // rather than leaving a progress-free wait.
          ({ sent, total }) => {
            if (total > 0 && sent < total) {
              setBusy(`Uploading ${file.name} — ${Math.floor((sent / total) * 100)}%`);
            }
          },
        );

        return result.ok
          ? { ok: true }
          : { ok: false, problem: result.problem };
      });

      if (ok) {
        uploaded += 1;
      }
    }

    if (uploaded > 0) {
      setNotice(`${uploaded} file${uploaded === 1 ? "" : "s"} uploaded.`);
    }
    if (uploadInput.current) {
      uploadInput.current.value = "";
    }
  }

  async function onNewFolder() {
    const name = window.prompt("Name for the new folder");
    if (!name?.trim()) {
      return;
    }

    const created = await run("Creating folder", async () => {
      const result = await createFolder(library, joinPath(path, name.trim()));

      return result.ok ? { ok: true } : { ok: false, problem: result.problem };
    });

    if (created) {
      setNotice(`Folder “${name.trim()}” created.`);
    }
  }

  async function onRename(item: Item) {
    const name = window.prompt(`Rename “${item.name}” to`, item.name);
    if (!name?.trim() || name.trim() === item.name) {
      return;
    }

    const renamed = await run("Renaming", async () => {
      const result = await moveItem(
        item.id,
        joinPath(parentOf(item.path), name.trim()),
      );

      return result.ok ? { ok: true } : { ok: false, problem: result.problem };
    });

    if (renamed) {
      setNotice(`Renamed to “${name.trim()}”.`);
    }
  }

  async function onTrash(item: Item) {
    const confirmed = window.confirm(
      `Move “${item.name}” to the trash? It stays on disk and can be restored from More.`,
    );
    if (!confirmed) {
      return;
    }

    const trashed = await run("Moving to trash", async () => {
      const result = await trashItem(item.id);

      return result.ok ? { ok: true } : { ok: false, problem: result.problem };
    });

    if (trashed) {
      setNotice(`“${item.name}” moved to the trash.`);
    }
  }

  if (state.phase === "loading") {
    return <PendingState label="Loading files…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="These files could not be loaded"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  const listing = state.data;

  return (
    <div>
      {requesting ? (
        <RequestDialog item={requesting} onClose={() => setRequesting(null)} />
      ) : null}

      {sharing ? (
        <ShareDialog item={sharing} onClose={() => setSharing(null)} />
      ) : null}

      <div className={styles.toolbar}>
        <Button
          variant="primary"
          onClick={() => uploadInput.current?.click()}
          disabled={Boolean(busy)}
        >
          Upload files
        </Button>
        <Button onClick={() => void onNewFolder()} disabled={Boolean(busy)}>
          New folder
        </Button>
        <input
          ref={uploadInput}
          className={styles.visuallyHidden}
          type="file"
          multiple
          aria-label="Choose files to upload"
          onChange={(event) => void onUpload(event.target.files)}
        />
      </div>

      <nav className={styles.breadcrumb} aria-label="Folder path">
        <ol className={styles.breadcrumbList}>
          <li>
            {path === "" ? (
              <span className={styles.current} aria-current="page">
                Library
              </span>
            ) : (
              <button
                type="button"
                className={styles.crumbButton}
                onClick={() => onNavigate("")}
              >
                Library
              </button>
            )}
          </li>
          {listing.breadcrumb.map((crumb, index) => {
            const isCurrent = index === listing.breadcrumb.length - 1;

            return (
              <li key={crumb.path}>
                <span className={styles.breadcrumbSeparator} aria-hidden="true">
                  {" / "}
                </span>
                {isCurrent ? (
                  <span className={styles.current} aria-current="page">
                    {crumb.name}
                  </span>
                ) : (
                  <button
                    type="button"
                    className={styles.crumbButton}
                    onClick={() => onNavigate(crumb.path)}
                  >
                    {crumb.name}
                  </button>
                )}
              </li>
            );
          })}
        </ol>
      </nav>

      {busy ? <PendingState label={`${busy}…`} /> : null}
      {notice ? (
        <p className={styles.status} role="status">
          {notice}
        </p>
      ) : null}
      {problem ? (
        <ErrorState title="That did not work" description={problem.detail} />
      ) : null}

      {listing.items.length === 0 ? (
        <EmptyState
          title="This folder is empty"
          description="Upload files, or copy them into the library folder on the server and run a scan from More."
        />
      ) : (
        <div className={styles.tableWrap}>
          <table className={styles.table}>
            <caption>
              {listing.items.length} item{listing.items.length === 1 ? "" : "s"}
            </caption>
            <thead>
              <tr>
                <th scope="col">Name</th>
                <th scope="col" className={styles.hideNarrow}>
                  Size
                </th>
                <th scope="col" className={styles.hideNarrow}>
                  Modified
                </th>
                <th scope="col">
                  <span className={styles.visuallyHidden}>Actions</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {listing.items.map((item) => (
                <tr key={item.id}>
                  <td className={styles.nameCell}>
                    {item.kind === "folder" ? (
                      <button
                        type="button"
                        className={styles.name}
                        onClick={() => onNavigate(item.path)}
                      >
                        <span className={styles.kindIcon} aria-hidden="true">
                          📁
                        </span>
                        {item.name}
                      </button>
                    ) : (
                      <span className={styles.name}>
                        {item.isImage || item.isVideo ? (
                          // The optimizer cannot reach a private,
                          // session-protected origin.
                          // eslint-disable-next-line @next/next/no-img-element
                          <img
                            className={styles.preview}
                            src={thumbnailUrl(item.id, "small")}
                            alt=""
                            loading="lazy"
                            decoding="async"
                          />
                        ) : (
                          <span className={styles.kindIcon} aria-hidden="true">
                            📄
                          </span>
                        )}
                        {item.name}
                      </span>
                    )}
                  </td>
                  <td className={`${styles.numeric} ${styles.hideNarrow}`}>
                    {item.kind === "folder" ? "—" : formatBytes(item.sizeBytes)}
                  </td>
                  <td className={`${styles.numeric} ${styles.hideNarrow}`}>
                    <time dateTime={item.modifiedAt ?? undefined}>
                      {formatDate(item.modifiedAt)}
                    </time>
                  </td>
                  <td>
                    <div className={styles.rowActions}>
                      {item.kind === "file" ? (
                        <a
                          className={styles.downloadLink}
                          href={contentUrl(item.id)}
                          download={item.name}
                        >
                          Download
                          <span className={styles.visuallyHidden}>
                            {" "}
                            {item.name}
                          </span>
                        </a>
                      ) : null}
                      <Button variant="quiet" onClick={() => setSharing(item)}>
                        Share
                        <span className={styles.visuallyHidden}>
                          {" "}
                          {item.name}
                        </span>
                      </Button>
                      {item.kind === "folder" ? (
                        <Button variant="quiet" onClick={() => setRequesting(item)}>
                          Ask for files
                          <span className={styles.visuallyHidden}>
                            {" "}
                            {item.name}
                          </span>
                        </Button>
                      ) : null}
                      <Button
                        variant="quiet"
                        onClick={() => void onRename(item)}
                      >
                        Rename
                        <span className={styles.visuallyHidden}>
                          {" "}
                          {item.name}
                        </span>
                      </Button>
                      <Button
                        variant="quiet"
                        onClick={() => void onTrash(item)}
                      >
                        Delete
                        <span className={styles.visuallyHidden}>
                          {" "}
                          {item.name}
                        </span>
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
