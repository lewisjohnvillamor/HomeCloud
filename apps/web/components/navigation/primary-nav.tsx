"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { isCurrent, NAVIGATION_ITEMS } from "./navigation-items";
import styles from "./primary-nav.module.css";

/**
 * The application's only primary navigation. It is rendered once and
 * restyled per device class: a sidebar on wide viewports, a bottom bar on
 * narrow ones. Rendering two copies would duplicate the links for screen
 * reader and keyboard users.
 */
export function PrimaryNav() {
  const pathname = usePathname();

  return (
    <nav className={styles.nav} aria-label="Primary">
      <ul className={styles.list}>
        {NAVIGATION_ITEMS.map((item) => {
          const current = isCurrent(pathname, item.href);

          return (
            <li key={item.href} className={styles.item}>
              <Link
                href={item.href}
                className={styles.link}
                aria-current={current ? "page" : undefined}
                data-current={current ? "true" : undefined}
              >
                {item.label}
              </Link>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
