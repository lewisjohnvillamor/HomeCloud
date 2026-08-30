import { beforeEach, describe, expect, it, vi } from "vitest";
import { RESUMABLE_THRESHOLD_BYTES, sendFile } from "@/lib/api/send-file";

const endpoints = vi.hoisted(() => ({
  uploadFile: vi.fn(),
  createUploadSession: vi.fn(),
  appendUploadChunk: vi.fn(),
  fetchUploadStatus: vi.fn(),
  completeUpload: vi.fn(),
}));

vi.mock("@/lib/api/endpoints", () => endpoints);

/** A file of a given size, without allocating one that large for real. */
function file(size: number, name = "clip.mp4"): File {
  const blob = new Blob([new Uint8Array(Math.min(size, 1024))]);
  const handle = new File([blob], name);

  Object.defineProperty(handle, "size", { value: size });
  // jsdom's slice ignores the faked size, and the bytes are not what is
  // under test here — the offsets are.
  Object.defineProperty(handle, "slice", {
    value: (start: number, end: number) => ({ start, end }),
  });

  return handle;
}

/** A chunk size that divides the test files into a readable few. */
const CHUNK = 4_000_000;

function session(overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    data: {
      id: "session-1",
      path: "clip.mp4",
      offset: 0,
      sizeBytes: 0,
      maxChunkBytes: CHUNK,
      expiresAt: "",
      ...overrides,
    },
  };
}

beforeEach(() => vi.clearAllMocks());

describe("sendFile", () => {
  it("sends a small file in one request rather than opening a session", async () => {
    endpoints.uploadFile.mockResolvedValue({ ok: true, data: { id: "item" } });

    const result = await sendFile({
      library: "lib",
      path: "photo.png",
      file: file(RESUMABLE_THRESHOLD_BYTES),
    });

    expect(result.ok).toBe(true);
    expect(endpoints.uploadFile).toHaveBeenCalledOnce();
    expect(endpoints.createUploadSession).not.toHaveBeenCalled();
  });

  it("sends a large file in chunks and completes it", async () => {
    endpoints.createUploadSession.mockResolvedValue(session());
    let offset = 0;
    endpoints.appendUploadChunk.mockImplementation((_id, at: number, chunk: {start: number; end: number}) => {
      expect(at).toBe(offset);
      offset = chunk.end;
      return Promise.resolve(session({ offset }));
    });
    endpoints.completeUpload.mockResolvedValue({ ok: true, data: { id: "item" } });

    const progress: number[] = [];
    const result = await sendFile(
      { library: "lib", path: "clip.mp4", file: file(10_000_000) },
      ({ sent }) => progress.push(sent),
    );

    expect(result.ok).toBe(true);
    expect(endpoints.appendUploadChunk).toHaveBeenCalledTimes(3);
    expect(progress).toEqual([0, 4_000_000, 8_000_000, 10_000_000]);
    expect(endpoints.completeUpload).toHaveBeenCalledOnce();
  });

  it("asks the server where it got to after a failed chunk, and continues from there", async () => {
    endpoints.createUploadSession.mockResolvedValue(session());

    // The first chunk fails, but the server had in fact received it: the
    // response was lost, not the bytes. Continuing from a locally kept
    // count would send those bytes twice.
    endpoints.appendUploadChunk
      .mockResolvedValueOnce({ ok: false, problem: { code: "network", detail: "gone" } })
      .mockImplementation((_id, _at: number, chunk: { start: number; end: number }) =>
        Promise.resolve(session({ offset: chunk.end })),
      );
    endpoints.fetchUploadStatus.mockResolvedValue(session({ offset: CHUNK }));
    endpoints.completeUpload.mockResolvedValue({ ok: true, data: { id: "item" } });

    const result = await sendFile({
      library: "lib",
      path: "clip.mp4",
      file: file(9_000_000),
    });

    expect(result.ok).toBe(true);
    expect(endpoints.fetchUploadStatus).toHaveBeenCalledOnce();
    // The retry continues from what the server has, not from where this
    // client thought it was: the bytes arrived, the response did not.
    const offsets = endpoints.appendUploadChunk.mock.calls.map((call) => call[1]);
    expect(offsets).toEqual([0, CHUNK, 2 * CHUNK]);
  });

  it("gives up rather than retrying forever when nothing is getting through", async () => {
    endpoints.createUploadSession.mockResolvedValue(session());
    endpoints.appendUploadChunk.mockResolvedValue({
      ok: false,
      problem: { code: "network", detail: "gone" },
    });
    endpoints.fetchUploadStatus.mockResolvedValue(session({ offset: 0 }));

    const result = await sendFile({
      library: "lib",
      path: "clip.mp4",
      file: file(9_000_000),
    });

    expect(result.ok).toBe(false);
    expect(endpoints.completeUpload).not.toHaveBeenCalled();
  });

  it("resumes a session the server says is already part-way through", async () => {
    endpoints.createUploadSession.mockResolvedValue(session({ offset: 7_000_000 }));
    endpoints.appendUploadChunk.mockImplementation(
      (_id, at: number, chunk: {start: number; end: number}) => {
        expect(at).toBe(7_000_000);
        return Promise.resolve(session({ offset: chunk.end }));
      },
    );
    endpoints.completeUpload.mockResolvedValue({ ok: true, data: { id: "item" } });

    // Already most of the way there: only the tail is left to send.
    const result = await sendFile({
      library: "lib",
      path: "clip.mp4",
      file: file(9_000_000),
    });

    expect(result.ok).toBe(true);
    expect(endpoints.appendUploadChunk).toHaveBeenCalledOnce();
  });

  it("reports a refused session rather than uploading anyway", async () => {
    endpoints.createUploadSession.mockResolvedValue({
      ok: false,
      problem: { code: "payload_too_large", detail: "too big" },
    });

    const result = await sendFile({
      library: "lib",
      path: "clip.mp4",
      file: file(9_000_000),
    });

    expect(result.ok).toBe(false);
    expect(endpoints.appendUploadChunk).not.toHaveBeenCalled();
  });
});
