import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PhotoGrid } from "@/components/photos/photo-grid";
import type { Item } from "@/lib/api/types";

const endpoints = vi.hoisted(() => ({
  fetchPhotos: vi.fn(),
  contentUrl: (id: string) => `/content/${id}`,
  thumbnailUrl: (id: string) => `/thumb/${id}`,
}));

vi.mock("@/lib/api/endpoints", () => endpoints);

function media(name: string, overrides: Partial<Item> = {}): Item {
  return {
    id: `id-${name}`,
    name,
    path: name,
    kind: "file",
    sizeBytes: 1000,
    contentType: "image/png",
    modifiedAt: "2026-03-04T10:00:00Z",
    isImage: true,
    isVideo: false,
    trashed: false,
    ...overrides,
  };
}

afterEach(() => vi.clearAllMocks());

describe("PhotoGrid", () => {
  it("counts photos and videos separately rather than calling everything a photo", async () => {
    endpoints.fetchPhotos.mockResolvedValue({
      ok: true,
      data: [
        media("beach.png"),
        media("clip.mp4", { isImage: false, isVideo: true, contentType: "video/mp4" }),
      ],
    });

    render(<PhotoGrid library="lib-1" />);

    expect(await screen.findByText("1 photo · 1 video")).toBeInTheDocument();
  });

  it("marks a video so it does not look like a still", async () => {
    endpoints.fetchPhotos.mockResolvedValue({
      ok: true,
      data: [media("clip.mp4", { isImage: false, isVideo: true, contentType: "video/mp4" })],
    });

    render(<PhotoGrid library="lib-1" />);

    expect(await screen.findByText("Video · clip.mp4")).toBeInTheDocument();
  });

  it("groups by the month a photo was taken", async () => {
    endpoints.fetchPhotos.mockResolvedValue({
      ok: true,
      data: [
        media("march.png", { modifiedAt: "2026-03-04T10:00:00Z" }),
        media("april.png", { modifiedAt: "2026-04-04T10:00:00Z" }),
      ],
    });

    render(<PhotoGrid library="lib-1" />);

    const headings = await screen.findAllByRole("heading", { level: 2 });
    // Newest month first.
    expect(headings[0]).toHaveTextContent("April 2026");
    expect(headings[1]).toHaveTextContent("March 2026");
  });

  it("says plainly when there is nothing to show", async () => {
    endpoints.fetchPhotos.mockResolvedValue({ ok: true, data: [] });

    render(<PhotoGrid library="lib-1" />);

    expect(await screen.findByRole("heading", { name: "No photos yet" })).toBeInTheDocument();
  });
});
