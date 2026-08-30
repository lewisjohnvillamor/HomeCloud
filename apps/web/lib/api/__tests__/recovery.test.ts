import { describe, expect, it } from "vitest";
import { publicQuery } from "@/lib/api/endpoints";
import { toProblem } from "@/lib/api/problem";
import { parseSession, parseShare } from "@/lib/api/types";

describe("publicQuery", () => {
  it("is empty when a link needs neither an item nor a key", () => {
    expect(publicQuery(undefined, null)).toBe("");
  });

  it("carries the unlock key, because an <img> cannot send a header", () => {
    expect(publicQuery(undefined, "abc def")).toBe("?key=abc%20def");
  });

  it("combines an item with a key", () => {
    expect(publicQuery("item-1", "k")).toBe("?item=item-1&key=k");
  });
});

describe("password_required", () => {
  it("survives parsing, so the unlock screen can be told apart from a refusal", () => {
    const problem = toProblem(401, {
      code: "password_required",
      detail: "This link is password protected.",
    });

    expect(problem.code).toBe("password_required");
  });
});

describe("parseShare", () => {
  it("reads whether a link is protected", () => {
    const share = parseShare({ id: "s", item_id: "i", protected: true });

    expect(share?.protected).toBe(true);
  });

  it("treats a missing flag as unprotected rather than failing", () => {
    expect(parseShare({ id: "s", item_id: "i" })?.protected).toBe(false);
  });
});

describe("parseSession", () => {
  it("keeps a recovery code that is shown once", () => {
    const session = parseSession({
      authenticated: true,
      user_id: "u",
      display_name: "Ada",
      recovery_code: "ABCD-EFGH",
    });

    expect(session?.recoveryCode).toBe("ABCD-EFGH");
  });

  it("has no code on an ordinary session read", () => {
    expect(parseSession({ authenticated: false })?.recoveryCode).toBeNull();
  });
});
