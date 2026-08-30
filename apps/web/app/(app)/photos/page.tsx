"use client";

import { PhotosView } from "@/components/photos/photos-view";
import { useActiveLibrary } from "@/components/session/session-provider";
import { EmptyState } from "@/components/ui/states";

export default function PhotosPage() {
  const library = useActiveLibrary();

  return (
    <>
      <h1>Photos</h1>
      {library ? (
        <PhotosView library={library.id} />
      ) : (
        <EmptyState
          title="No library yet"
          description="This account is not a member of any library."
        />
      )}
    </>
  );
}
