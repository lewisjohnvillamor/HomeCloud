import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => {
  cleanup();
});

// jsdom hides `[popover]` elements, as a browser does, but implements none
// of the API that opens them. This stands in for the part components use:
// `popovertarget` buttons toggle their popover, the methods exist, and a
// `toggle` event carries `newState`, so a menu can be tested by clicking it
// open the way a person would.
if (typeof HTMLElement !== "undefined" && !("showPopover" in HTMLElement.prototype)) {
  const OPEN = "data-test-popover-open";
  const style = document.createElement("style");
  style.textContent = `[popover][${OPEN}] { display: block !important; }`;
  document.head.append(style);

  const setOpen = (element: HTMLElement, open: boolean) => {
    if (element.hasAttribute(OPEN) === open) return;
    element.toggleAttribute(OPEN, open);
    const event = Object.assign(new Event("toggle"), { newState: open ? "open" : "closed" });
    element.dispatchEvent(event);
  };

  Object.assign(HTMLElement.prototype, {
    showPopover(this: HTMLElement) {
      setOpen(this, true);
    },
    hidePopover(this: HTMLElement) {
      setOpen(this, false);
    },
    togglePopover(this: HTMLElement) {
      setOpen(this, !this.hasAttribute(OPEN));
    },
  });

  document.addEventListener("click", (event) => {
    const invoker = (event.target as Element | null)?.closest?.("[popovertarget]");
    const id = invoker?.getAttribute("popovertarget");
    const target = id ? document.getElementById(id) : null;
    if (target) setOpen(target, !target.hasAttribute(OPEN));
  });
}
