import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PhotoWall, type WallSource } from "../photo-wall";
import type { Item, MemoryGroup } from "@/lib/api/types";

function photo(name: string): Item {
  return {
    id: `id-${name}`,
    name,
    path: name,
    kind: "file",
    sizeBytes: 1000,
    contentType: "image/jpeg",
    modifiedAt: "2026-06-01T10:00:00Z",
    takenAt: "2026-06-01T10:00:00Z",
    camera: null,
    latitude: null,
    longitude: null,
    isImage: true,
    isVideo: false,
    trashed: false,
  };
}

const MEMORIES: MemoryGroup[] = [
  {
    key: "recently-added",
    title: "Recently added",
    subtitle: "2 photos",
    items: [photo("one.jpg"), photo("two.jpg")],
  },
];

const source: WallSource = {
  memories: () => Promise.resolve({ ok: true, data: MEMORIES }),
  thumbnail: (item) => `/thumb/${item}`,
  content: (item) => `/content/${item}`,
};

function renderWall() {
  return render(<PhotoWall source={source} />);
}

describe("photo frame", () => {
  it("starts on the play/pause key and shows one photo with no chrome", async () => {
    renderWall();
    await screen.findByRole("heading", { name: "Photos" });

    // The remote's play/pause key: the one control a photo frame needs.
    fireEvent.keyDown(window, { key: "MediaPlayPause" });

    expect(await screen.findByRole("img", { name: /Photo frame/ })).toBeInTheDocument();

    // Nothing to read from across a room: no wall, no captions, no hint.
    expect(screen.queryByRole("heading", { name: "Photos" })).not.toBeInTheDocument();
    expect(screen.queryByText(/Arrows to move/)).not.toBeInTheDocument();
  });

  it("leaves on any key, because somebody has picked up the remote", async () => {
    renderWall();
    await screen.findByRole("heading", { name: "Photos" });

    fireEvent.keyDown(window, { key: "MediaPlayPause" });
    await screen.findByRole("img", { name: /Photo frame/ });

    fireEvent.keyDown(window, { key: "ArrowRight" });

    expect(await screen.findByRole("heading", { name: "Photos" })).toBeInTheDocument();
  });

  it("shows the time, because a frame on a shelf is also a clock", async () => {
    renderWall();
    await screen.findByRole("heading", { name: "Photos" });

    fireEvent.keyDown(window, { key: "MediaPlayPause" });
    await screen.findByRole("img", { name: /Photo frame/ });

    // Set when the frame is entered rather than on the first tick, so
    // the corner is not blank for the first minute.
    expect(screen.getByText(/\d{1,2}[:.]\d{2}/)).toBeInTheDocument();
  });
});
