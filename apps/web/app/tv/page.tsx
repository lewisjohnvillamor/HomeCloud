"use client";

import { PhotoWall } from "@/components/tv/photo-wall";
import { useActiveLibrary } from "@/components/session/session-provider";

export default function TvPage() {
  const library = useActiveLibrary();

  if (!library) {
    return <h1>No library on this account</h1>;
  }

  return <PhotoWall library={library.id} />;
}
