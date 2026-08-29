import { EmptyState } from "@/components/ui/states";

export default function HomePage() {
  return (
    <>
      <h1>HomeCloud</h1>
      <EmptyState
        title="This deployment is not set up yet"
        description="Once the server is reachable and a library folder is configured, your files and photos appear here."
      />
    </>
  );
}
