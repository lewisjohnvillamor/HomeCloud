import type { Metadata } from "next";
import { EmptyState } from "@/components/ui/states";

export const metadata: Metadata = { title: "Photos" };

export default function PhotosPage() {
  return (
    <>
      <h1>Photos</h1>
      <EmptyState
        title="Nothing here yet"
        description="No photos have been indexed yet. Photos appear once a library folder is scanned."
      />
    </>
  );
}
