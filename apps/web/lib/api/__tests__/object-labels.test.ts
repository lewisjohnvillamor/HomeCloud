import { describe, expect, it } from "vitest";
import { parseAiSettings, parseSearchResults } from "@/lib/api/types";

const photo = {
  id: "item-1",
  name: "IMG_0001.jpg",
  path: "park/IMG_0001.jpg",
  kind: "file",
};

describe("parseSearchResults", () => {
  it("keeps what was recognised in a photo, so it can be shown as an inference", () => {
    const [hit] = parseSearchResults([
      { ...photo, matched: "objects", objects: ["dog", "person"] },
    ])!;

    expect(hit?.matched).toBe("objects");
    expect(hit?.objects).toEqual(["dog", "person"]);
  });

  it("gives every other hit an empty list rather than undefined", () => {
    const [hit] = parseSearchResults([{ ...photo, matched: "name" }])!;
    expect(hit?.objects).toEqual([]);
  });

  it("drops labels that are not text instead of rendering them", () => {
    const [hit] = parseSearchResults([
      { ...photo, matched: "objects", objects: ["dog", 4, null, { html: "<b>" }] },
    ])!;
    expect(hit?.objects).toEqual(["dog"]);
  });
});

describe("parseAiSettings", () => {
  it("reads whether this server can recognise what is in photos", () => {
    const settings = parseAiSettings({
      profile: "photos",
      ocr_available: true,
      objects_available: true,
      supported_profile: "photos",
      pending_items: 3,
      objects_pending: 120,
    })!;

    expect(settings.objectsAvailable).toBe(true);
    expect(settings.objectsPending).toBe(120);
  });

  it("treats an older server that does not mention objects as not having them", () => {
    const settings = parseAiSettings({
      profile: "text",
      ocr_available: true,
      supported_profile: "text",
      pending_items: 0,
    })!;

    expect(settings.objectsAvailable).toBe(false);
    expect(settings.objectsPending).toBe(0);
  });
});
