import type { Metadata } from "next";
import { EmptyState } from "@/components/ui/states";

export const metadata: Metadata = { title: "Search" };

export default function SearchPage() {
  return (
    <>
      <h1>Search</h1>
      <EmptyState
        title="Nothing here yet"
        description="Search runs against the catalog. Nothing is indexed yet, so there is nothing to search."
      />
    </>
  );
}
