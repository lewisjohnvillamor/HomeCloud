import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import "./globals.css";

export const metadata: Metadata = {
  title: {
    default: "HomeCloud",
    template: "%s · HomeCloud",
  },
  description: "Your files and photos, on hardware you own.",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
};

/**
 * Only the document. The application shell and the public share view are
 * separate layouts, because a visitor holding a share link must not be
 * shown navigation into a library they cannot open.
 */
export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
