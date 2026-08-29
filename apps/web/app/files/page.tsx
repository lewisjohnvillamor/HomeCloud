import type { Metadata } from "next";
import { EmptyState } from "@/components/ui/states";

export const metadata: Metadata = { title: "Files" };

export default function FilesPage() {
  return (
    <>
      <h1>Files</h1>
      <EmptyState
        title="Nothing here yet"
        description="Nothing is indexed yet. Connect a library folder and the catalog will list its contents here."
      />
    </>
  );
}
