import type { IconName } from "@/components/ui/icon";

/**
 * The primary destinations. One list drives the desktop sidebar and the
 * mobile bar so the two can never drift apart.
 */
export type NavigationItem = {
  href: string;
  label: string;
  /** Short label used where horizontal space is tight. */
  shortLabel: string;
  icon: IconName;
};

export const NAVIGATION_ITEMS: readonly NavigationItem[] = [
  { href: "/", label: "Home", shortLabel: "Home", icon: "home" },
  { href: "/files", label: "Files", shortLabel: "Files", icon: "files" },
  { href: "/photos", label: "Photos", shortLabel: "Photos", icon: "photos" },
  { href: "/search", label: "Search", shortLabel: "Search", icon: "search" },
  { href: "/more", label: "More", shortLabel: "More", icon: "more" },
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
