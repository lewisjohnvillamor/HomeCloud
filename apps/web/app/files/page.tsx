"use client";

import { Suspense } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { FileBrowser } from "@/components/files/file-browser";
import { useActiveLibrary } from "@/components/session/session-provider";
import { EmptyState, PendingState } from "@/components/ui/states";

function Files() {
  const library = useActiveLibrary();
  const router = useRouter();
  const params = useSearchParams();
  const path = params.get("path") ?? "";

  if (!library) {
    return (
      <EmptyState
        title="No library yet"
        description="This account is not a member of any library."
      />
    );
  }

  return (
    <FileBrowser
      library={library.id}
      path={path}
      onNavigate={(next) => {
        // The folder lives in the URL so back, forward, and sharing a
        // link all work the way they do everywhere else.
        router.push(next ? `/files?path=${encodeURIComponent(next)}` : "/files");
      }}
    />
  );
}

export default function FilesPage() {
  return (
    <>
      <h1>Files</h1>
      <Suspense fallback={<PendingState label="Loading files…" />}>
        <Files />
      </Suspense>
    </>
  );
}
