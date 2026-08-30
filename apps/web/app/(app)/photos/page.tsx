"use client";

import { PhotoGrid } from "@/components/photos/photo-grid";
import { useActiveLibrary } from "@/components/session/session-provider";
import { EmptyState } from "@/components/ui/states";

export default function PhotosPage() {
  const library = useActiveLibrary();

  return (
    <>
      <h1>Photos</h1>
      {library ? (
        <PhotoGrid library={library.id} />
      ) : (
        <EmptyState
          title="No library yet"
          description="This account is not a member of any library."
        />
      )}
    </>
  );
}
