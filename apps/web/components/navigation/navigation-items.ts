/**
 * The primary destinations. One list drives the desktop sidebar and the
 * mobile bar so the two can never drift apart.
 */
export type NavigationItem = {
  href: string;
  label: string;
  /** Short label used where horizontal space is tight. */
  shortLabel: string;
};

export const NAVIGATION_ITEMS: readonly NavigationItem[] = [
  { href: "/", label: "Home", shortLabel: "Home" },
  { href: "/files", label: "Files", shortLabel: "Files" },
  { href: "/photos", label: "Photos", shortLabel: "Photos" },
  { href: "/search", label: "Search", shortLabel: "Search" },
  { href: "/more", label: "More", shortLabel: "More" },
] as const;

/**
 * Marks the deepest matching destination as current so `/files/holiday`
 * still highlights Files without also highlighting Home.
 */
export function isCurrent(pathname: string, href: string): boolean {
  if (href === "/") {
    return pathname === "/";
  }

  return pathname === href || pathname.startsWith(`${href}/`);
}
