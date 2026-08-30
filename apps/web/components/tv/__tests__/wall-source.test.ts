import { describe, expect, it } from "vitest";
import { pairedSource, sessionSource } from "../photo-wall";

describe("wall sources", () => {
  it("a paired screen carries its credential on every picture", () => {
    const source = pairedSource("device-token");

    expect(source.thumbnail("item-1")).toContain("token=device-token");
    expect(source.thumbnail("item-1")).toContain("item=item-1");
    expect(source.content("item-1")).toContain("/api/v1/tv/content");
  });

  it("escapes a token rather than pasting it into a URL", () => {
    expect(pairedSource("a b&c").thumbnail("x")).toContain("token=a%20b%26c");
  });

  it("a signed-in screen uses the ordinary session routes", () => {
    const source = sessionSource("library-1");

    expect(source.thumbnail("item-1")).not.toContain("token=");
    expect(source.content("item-1")).toContain("/api/v1/items/item-1/content");
  });
});
