"use client";

import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode } from "react";
import { Icon, type IconName } from "@/components/ui/icon";
import { buttonStyles } from "@/components/ui/button";
import styles from "./action-menu.module.css";

export type MenuAction = {
  key: string;
  label: string;
  icon: IconName;
  onSelect: () => void;
  /** Shown only where the row has no room for it inline (phones). */
  narrowOnly?: boolean;
  /** Destructive: set apart in colour, and always named in words. */
  danger?: boolean;
};

/**
 * A row's less frequent actions, behind one "More" button.
 *
 * A file row offered six verbs inline. On a phone they stacked into a
 * block taller than the screen per file; on a desktop a long name pushed
 * the last of them off the edge. The common verbs stay on the row; the
 * rest are a tap away here.
 *
 * Built on the platform popover: it sits in the top layer, so no sticky
 * header or bottom bar can cover it, it closes on Escape or an outside
 * click, and focus returns to the button that opened it.
 */
export function ActionMenu({
  name,
  actions,
  trigger = "More",
}: {
  /** What the actions apply to, for the button's accessible name. */
  name: string;
  actions: MenuAction[];
  trigger?: ReactNode;
}) {
  const id = useId();
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = menu.current;
    if (!element) return;

    function onToggle(event: Event) {
      const open = (event as ToggleEvent).newState === "open";
      button.current?.setAttribute("aria-expanded", String(open));
      if (!open || !element || !button.current) return;

      // Below the button, right edges aligned, flipped above when there
      // is no room underneath — the anchor positioning CSS that would do
      // this is not in every browser yet.
      const anchor = button.current.getBoundingClientRect();
      const box = element.getBoundingClientRect();
      const gap = 4;
      const left = Math.max(8, Math.min(anchor.right - box.width, innerWidth - box.width - 8));
      const below = anchor.bottom + gap;
      const top = below + box.height > innerHeight - 8 ? anchor.top - gap - box.height : below;
      element.style.left = `${left}px`;
      element.style.top = `${Math.max(8, top)}px`;

      items(element)[0]?.focus();
    }

    element.addEventListener("toggle", onToggle);
    return () => element.removeEventListener("toggle", onToggle);
  }, []);

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const list = items(event.currentTarget);
    const index = list.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      event.key === "ArrowDown"
        ? (index + 1) % list.length
        : event.key === "ArrowUp"
          ? (index - 1 + list.length) % list.length
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? list.length - 1
              : null;

    if (next !== null) {
      event.preventDefault();
      list[next]?.focus();
    }
  }

  function choose(action: MenuAction) {
    menu.current?.hidePopover?.();
    action.onSelect();
  }

  return (
    <>
      <button
        ref={button}
        type="button"
        className={`${buttonStyles.button} ${buttonStyles.quiet} ${styles.trigger}`}
        popoverTarget={id}
        aria-haspopup="menu"
        aria-expanded="false"
      >
        <Icon name="more" />
        <span className={styles.triggerLabel}>{trigger}</span>
        <span className={styles.visuallyHidden}> actions for {name}</span>
      </button>
      <div
        ref={menu}
        id={id}
        popover="auto"
        role="menu"
        aria-label={`Actions for ${name}`}
        className={styles.menu}
        onKeyDown={onKeyDown}
      >
        {actions.map((action) => (
          <button
            key={action.key}
            type="button"
            role="menuitem"
            className={styles.item}
            data-narrow-only={action.narrowOnly ? "true" : undefined}
            data-danger={action.danger ? "true" : undefined}
            onClick={() => choose(action)}
          >
            <Icon name={action.icon} />
            {action.label}
          </button>
        ))}
      </div>
    </>
  );
}

/** The items a person can actually reach: hidden ones are skipped. */
function items(menu: HTMLElement): HTMLButtonElement[] {
  return [...menu.querySelectorAll<HTMLButtonElement>("[role=menuitem]")].filter(
    (item) => getComputedStyle(item).display !== "none",
  );
}
