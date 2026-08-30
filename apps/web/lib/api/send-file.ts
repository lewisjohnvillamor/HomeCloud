/**
 * Sending one file to the server, whichever way suits its size.
 *
 * A photo goes in one request: a session would be three round trips to
 * move two megabytes. A video goes through a resumable session, because
 * the thing that actually happens on house wifi is that a long upload is
 * interrupted, and starting a 40 GB file again is not an option.
 *
 * The offset always comes from the server, never from a count kept here:
 * what survived a dropped connection is a fact about the server's disk,
 * and guessing it is how a resumable upload corrupts a file.
 */

import {
  appendUploadChunk,
  completeUpload,
  createUploadSession,
  fetchUploadStatus,
  uploadFile,
} from "./endpoints";
import type { ApiResult } from "./client";
import type { Item } from "./types";

/**
 * Above this, a file is sent as a session. Below it, one request is
 * simpler and faster.
 */
export const RESUMABLE_THRESHOLD_BYTES = 8 * 1024 * 1024;

/** How many times a stalled chunk is retried before giving up. */
const MAX_RETRIES = 3;

export type UploadProgress = {
  /** Bytes the server has confirmed. */
  sent: number;
  total: number;
};

export async function sendFile(
  input: { library: string; path: string; file: File },
  onProgress?: (progress: UploadProgress) => void,
  signal?: AbortSignal,
): Promise<ApiResult<Item>> {
  const { library, path, file } = input;

  if (file.size <= RESUMABLE_THRESHOLD_BYTES) {
    onProgress?.({ sent: 0, total: file.size });
    const result = await uploadFile(library, path, file, { signal });

    if (result.ok) {
      onProgress?.({ sent: file.size, total: file.size });
    }

    return result;
  }

  const opened = await createUploadSession(
    { library, path, sizeBytes: file.size },
    { signal },
  );
  if (!opened.ok) {
    return opened;
  }

  const session = opened.data.id;
  const chunkSize = Math.max(1, opened.data.maxChunkBytes);
  let offset = opened.data.offset;
  let attempts = 0;

  onProgress?.({ sent: offset, total: file.size });

  while (offset < file.size) {
    const end = Math.min(offset + chunkSize, file.size);
    const sent = await appendUploadChunk(session, offset, file.slice(offset, end), {
      signal,
    });

    if (sent.ok) {
      offset = sent.data.offset;
      attempts = 0;
      onProgress?.({ sent: offset, total: file.size });
      continue;
    }

    // The connection dropped, or this client and the server disagree
    // about where they are. Both are answered the same way: ask the
    // server what it actually has, and continue from there.
    attempts += 1;
    if (attempts > MAX_RETRIES) {
      return sent;
    }

    const status = await fetchUploadStatus(session, { signal });
    if (!status.ok) {
      return status;
    }

    if (status.data.offset === offset) {
      // No progress at all this time round; the next attempt is the
      // last chance before this is reported as a failure.
      continue;
    }

    offset = status.data.offset;
    onProgress?.({ sent: offset, total: file.size });
  }

  return completeUpload(session, { signal });
}
