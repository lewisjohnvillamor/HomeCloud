"use client";

import { BackupView } from "@/components/backup/backup-view";
import { useActiveLibrary } from "@/components/session/session-provider";
import { EmptyState } from "@/components/ui/states";

export default function BackupPage() {
  const library = useActiveLibrary();

  return (
    <>
      <h1>Back up this phone</h1>
      {library ? (
        <BackupView library={library.id} />
      ) : (
        <EmptyState
          title="No library yet"
          description="This account is not a member of any library."
        />
      )}
    </>
  );
}
