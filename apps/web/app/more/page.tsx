import type { Metadata } from "next";
import { EmptyState } from "@/components/ui/states";

export const metadata: Metadata = { title: "More" };

export default function MorePage() {
  return (
    <>
      <h1>More</h1>
      <EmptyState
        title="Nothing here yet"
        description="Devices, sharing, and settings appear here as they are implemented."
      />
    </>
  );
}
