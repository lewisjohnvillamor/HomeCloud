import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BackupView } from "@/components/backup/backup-view";
import type { BackupDevice } from "@/lib/api/types";

const endpoints = vi.hoisted(() => ({
  BACKUP_BATCH: 2000,
  fetchBackupDevices: vi.fn(),
  registerBackupDevice: vi.fn(),
  checkBackup: vi.fn(),
  finishBackup: vi.fn(),
  forgetBackupDevice: vi.fn(),
}));

const transfer = vi.hoisted(() => ({ sendFile: vi.fn() }));

vi.mock("@/lib/api/endpoints", () => endpoints);
vi.mock("@/lib/api/send-file", () => transfer);

const PHONE: BackupDevice = {
  id: "device-1",
  name: "Ada's phone",
  folder: "Phone backups/Ada's phone",
  lastBackupAt: null,
  photoCount: 0,
};

function photo(name: string, size = 10): File {
  return new File([new Uint8Array(size)], name, { type: "image/jpeg" });
}

beforeEach(() => {
  vi.clearAllMocks();
  endpoints.fetchBackupDevices.mockResolvedValue({ ok: true, data: [PHONE] });
  endpoints.finishBackup.mockResolvedValue({ ok: true, data: PHONE });
  transfer.sendFile.mockImplementation(
    async (input: { path: string; file: File }) => ({
      ok: true,
      data: { id: `id-${input.file.name}`, name: input.file.name },
    }),
  );
});

describe("backing up a phone", () => {
  it("says plainly that it does not run on its own", async () => {
    render(<BackupView library="lib-1" />);

    // The sentence that stops somebody assuming a background service
    // and quietly losing photographs.
    expect(await screen.findByText(/does not run on its own/i)).toBeInTheDocument();
  });

  it("sends only the photographs the server has not got", async () => {
    endpoints.checkBackup.mockResolvedValue({
      ok: true,
      data: {
        missing: ["new.jpg"],
        alreadyHere: 1,
        folder: PHONE.folder,
      },
    });

    render(<BackupView library="lib-1" />);
    const chooser = await screen.findByLabelText("Choose photos to back up");

    await userEvent.upload(chooser, [photo("old.jpg"), photo("new.jpg")]);

    await waitFor(() => expect(transfer.sendFile).toHaveBeenCalledTimes(1));
    expect(transfer.sendFile.mock.calls[0]?.[0]).toMatchObject({
      library: "lib-1",
      path: "Phone backups/Ada's phone/new.jpg",
    });

    // And it says what it skipped, so "nothing happened" reads as
    // "there was nothing to do" rather than as a failure.
    expect(await screen.findByText(/1 sent\. 1 were already here\./)).toBeInTheDocument();
  });

  it("sends nothing at all when the whole roll is already here", async () => {
    endpoints.checkBackup.mockResolvedValue({
      ok: true,
      data: { missing: [], alreadyHere: 2, folder: PHONE.folder },
    });

    render(<BackupView library="lib-1" />);
    const chooser = await screen.findByLabelText("Choose photos to back up");

    await userEvent.upload(chooser, [photo("one.jpg"), photo("two.jpg")]);

    expect(await screen.findByText(/Everything is already here/i)).toBeInTheDocument();
    expect(transfer.sendFile).not.toHaveBeenCalled();
  });

  it("names a photograph that did not send", async () => {
    endpoints.checkBackup.mockResolvedValue({
      ok: true,
      data: { missing: ["broken.jpg"], alreadyHere: 0, folder: PHONE.folder },
    });
    transfer.sendFile.mockResolvedValue({
      ok: false,
      problem: { title: "Too large", detail: "That file is larger than this server accepts." },
    });

    render(<BackupView library="lib-1" />);
    const chooser = await screen.findByLabelText("Choose photos to back up");

    await userEvent.upload(chooser, [photo("broken.jpg")]);

    const tray = await screen.findByRole("region", { name: "Transfers" });
    await waitFor(() => expect(tray).toHaveTextContent("broken.jpg"));
    expect(tray).toHaveTextContent("larger than this server accepts");
  });

  it("asks for a name before it can back anything up", async () => {
    endpoints.fetchBackupDevices.mockResolvedValue({ ok: true, data: [] });
    endpoints.registerBackupDevice.mockResolvedValue({ ok: true, data: PHONE });

    render(<BackupView library="lib-1" />);

    const name = await screen.findByLabelText(/what is this phone called/i);
    await userEvent.type(name, "Ada's phone");
    await userEvent.click(screen.getByRole("button", { name: "Set up backup" }));

    await waitFor(() =>
      expect(endpoints.registerBackupDevice).toHaveBeenCalledWith("lib-1", "Ada's phone"),
    );
  });

  it("says a phone has never backed up rather than inventing a date", async () => {
    render(<BackupView library="lib-1" />);

    expect(await screen.findByText("Never")).toBeInTheDocument();
  });
});
