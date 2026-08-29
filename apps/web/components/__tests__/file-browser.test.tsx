import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FileBrowser } from "@/components/files/file-browser";
import type { Browse, Item } from "@/lib/api/types";

const endpoints = vi.hoisted(() => ({
  browse: vi.fn(),
  createFolder: vi.fn(),
  moveItem: vi.fn(),
  trashItem: vi.fn(),
  uploadFile: vi.fn(),
  contentUrl: (id: string) => `/api/v1/items/${id}/content`,
}));

vi.mock("@/lib/api/endpoints", () => endpoints);

function file(name: string, overrides: Partial<Item> = {}): Item {
  return {
    id: `id-${name}`,
    name,
    path: name,
    kind: "file",
    sizeBytes: 1234,
    contentType: "text/plain",
    modifiedAt: "2026-03-04T10:00:00Z",
    isImage: false,
    trashed: false,
    ...overrides,
  };
}

function listing(items: Item[], breadcrumb: Browse["breadcrumb"] = []): Browse {
  return { folder: null, breadcrumb, items };
}

function renderBrowser(onNavigate = vi.fn()) {
  render(<FileBrowser library="lib-1" path="" onNavigate={onNavigate} />);

  return onNavigate;
}

beforeEach(() => {
  endpoints.browse.mockResolvedValue({ ok: true, data: listing([]) });
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("FileBrowser", () => {
  it("lists what the folder contains", async () => {
    endpoints.browse.mockResolvedValue({
      ok: true,
      data: listing([
        file("photos", { kind: "folder", id: "id-photos" }),
        file("notes.txt"),
      ]),
    });

    renderBrowser();

    const rows = await screen.findAllByRole("row");
    // One header row plus one per item.
    expect(rows).toHaveLength(3);
    expect(screen.getByRole("button", { name: "photos" })).toBeInTheDocument();
    expect(screen.getByText("1.2 kB")).toBeInTheDocument();
  });

  it("says plainly when a folder is empty rather than showing nothing", async () => {
    renderBrowser();

    expect(await screen.findByRole("heading", { name: "This folder is empty" })).toBeInTheDocument();
  });

  it("navigates into a folder from the keyboard", async () => {
    endpoints.browse.mockResolvedValue({
      ok: true,
      data: listing([file("photos", { kind: "folder", path: "photos" })]),
    });
    const onNavigate = renderBrowser();

    const folder = await screen.findByRole("button", { name: "photos" });
    folder.focus();
    await userEvent.keyboard("{Enter}");

    expect(onNavigate).toHaveBeenCalledWith("photos");
  });

  it("offers a download link for files but not folders", async () => {
    endpoints.browse.mockResolvedValue({
      ok: true,
      data: listing([file("notes.txt"), file("photos", { kind: "folder" })]),
    });

    renderBrowser();

    const links = await screen.findAllByRole("link");
    expect(links).toHaveLength(1);
    expect(links[0]).toHaveAttribute("href", "/api/v1/items/id-notes.txt/content");
    expect(links[0]).toHaveAttribute("download", "notes.txt");
  });

  it("uploads a chosen file and reloads the listing", async () => {
    endpoints.uploadFile.mockResolvedValue({ ok: true, data: file("report.txt") });
    renderBrowser();
    await screen.findByRole("heading", { name: "This folder is empty" });

    const input = screen.getByLabelText("Choose files to upload");
    await userEvent.upload(
      input,
      new File(["contents"], "report.txt", { type: "text/plain" }),
    );

    await waitFor(() =>
      expect(endpoints.uploadFile).toHaveBeenCalledWith(
        "lib-1",
        "report.txt",
        expect.any(File),
      ),
    );
    expect(await screen.findByRole("status")).toHaveTextContent("1 file uploaded");
    // Twice: once on mount, once after the upload.
    expect(endpoints.browse).toHaveBeenCalledTimes(2);
  });

  it("reports a failed upload without losing the listing", async () => {
    endpoints.uploadFile.mockResolvedValue({
      ok: false,
      problem: { code: "payload_too_large", detail: "That file is too large." },
    });
    renderBrowser();
    await screen.findByRole("heading", { name: "This folder is empty" });

    await userEvent.upload(
      screen.getByLabelText("Choose files to upload"),
      new File(["contents"], "huge.bin"),
    );

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("That file is too large.");
  });

  it("confirms before moving an item to the trash", async () => {
    endpoints.browse.mockResolvedValue({ ok: true, data: listing([file("notes.txt")]) });
    endpoints.trashItem.mockResolvedValue({ ok: true, data: file("notes.txt", { trashed: true }) });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    renderBrowser();

    const row = (await screen.findAllByRole("row"))[1];
    await userEvent.click(within(row!).getByRole("button", { name: /Delete/ }));

    expect(confirm).toHaveBeenCalled();
    expect(endpoints.trashItem).not.toHaveBeenCalled();

    confirm.mockReturnValue(true);
    await userEvent.click(within(row!).getByRole("button", { name: /Delete/ }));

    await waitFor(() => expect(endpoints.trashItem).toHaveBeenCalledWith("id-notes.txt"));
    confirm.mockRestore();
  });

  it("renames within the same folder", async () => {
    endpoints.browse.mockResolvedValue({
      ok: true,
      data: listing([file("notes.txt", { path: "documents/notes.txt" })]),
    });
    endpoints.moveItem.mockResolvedValue({ ok: true, data: file("renamed.txt") });
    const prompt = vi.spyOn(window, "prompt").mockReturnValue("renamed.txt");
    renderBrowser();

    const row = (await screen.findAllByRole("row"))[1];
    await userEvent.click(within(row!).getByRole("button", { name: /Rename/ }));

    await waitFor(() =>
      expect(endpoints.moveItem).toHaveBeenCalledWith("id-notes.txt", "documents/renamed.txt"),
    );
    prompt.mockRestore();
  });

  it("offers a retry when the listing cannot be loaded", async () => {
    endpoints.browse.mockResolvedValueOnce({
      ok: false,
      problem: { code: "dependency_unavailable", detail: "The database is not available." },
    });
    renderBrowser();

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The database is not available.");

    await userEvent.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByRole("heading", { name: "This folder is empty" })).toBeInTheDocument();
  });
});
