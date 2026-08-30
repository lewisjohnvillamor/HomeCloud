import { describe, expect, it } from "vitest";
import { moveSelection, remoteAction } from "@/components/tv/tv-remote";

function key(name: string): KeyboardEvent {
  return new KeyboardEvent("keydown", { key: name });
}

describe("remoteAction", () => {
  it("maps the four directions", () => {
    expect(remoteAction(key("ArrowLeft"))).toBe("left");
    expect(remoteAction(key("ArrowRight"))).toBe("right");
    expect(remoteAction(key("ArrowUp"))).toBe("up");
    expect(remoteAction(key("ArrowDown"))).toBe("down");
  });

  it("maps select, back, and play/pause the way a remote sends them", () => {
    expect(remoteAction(key("Enter"))).toBe("select");
    expect(remoteAction(key("Escape"))).toBe("back");
    expect(remoteAction(key("Backspace"))).toBe("back");
    expect(remoteAction(key(" "))).toBe("playPause");
    expect(remoteAction(key("MediaPlayPause"))).toBe("playPause");
  });

  it("ignores keys a remote does not have", () => {
    expect(remoteAction(key("a"))).toBeNull();
    expect(remoteAction(key("F5"))).toBeNull();
  });
});

describe("moveSelection", () => {
  const total = 7;
  const columns = 3;

  it("moves within a row", () => {
    expect(moveSelection(0, "right", total, columns)).toBe(1);
    expect(moveSelection(1, "left", total, columns)).toBe(0);
  });

  it("moves between rows by a whole column count", () => {
    expect(moveSelection(0, "down", total, columns)).toBe(3);
    expect(moveSelection(4, "up", total, columns)).toBe(1);
  });

  it("stops at the edges rather than wrapping", () => {
    expect(moveSelection(0, "left", total, columns)).toBe(0);
    expect(moveSelection(0, "up", total, columns)).toBe(0);
    expect(moveSelection(6, "right", total, columns)).toBe(6);
    expect(moveSelection(6, "down", total, columns)).toBe(6);
  });

  it("survives an empty wall", () => {
    expect(moveSelection(0, "right", 0, columns)).toBe(0);
  });
});
