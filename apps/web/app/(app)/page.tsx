"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useActiveLibrary, useSession } from "@/components/session/session-provider";
import { MemoriesSection } from "@/components/memories/memories-section";
import { EmptyState } from "@/components/ui/states";
import { browse, fetchPhotos } from "@/lib/api/endpoints";
import styles from "./home.module.css";

type Counts = { items: number | null; photos: number | null };

export default function HomePage() {
  const { state } = useSession();
  const library = useActiveLibrary();
  const [counts, setCounts] = useState<Counts>({ items: null, photos: null });

  const libraryId = library?.id ?? null;

  useEffect(() => {
    if (!libraryId) {
      return;
    }

    const controller = new AbortController();

    void Promise.all([
      browse(libraryId, "", { signal: controller.signal }),
      fetchPhotos(libraryId, { signal: controller.signal }),
    ]).then(([listing, photos]) => {
      if (controller.signal.aborted) {
        return;
      }

      setCounts({
        items: listing.ok ? listing.data.items.length : null,
        photos: photos.ok ? photos.data.length : null,
      });
    });

    return () => controller.abort();
  }, [libraryId]);

  const name = state.phase === "signed-in" ? state.session.displayName : null;

  return (
    <>
      <h1>{name ? `Welcome back, ${name}` : "HomeCloud"}</h1>

      {library ? (
        <>
          <p className={styles.lead}>
            <strong>{library.name}</strong> is stored on this server. Nothing here is
            uploaded anywhere else.
          </p>

          <section className={styles.memories} aria-labelledby="memories-heading">
            <h2 id="memories-heading" className={styles.sectionHeading}>
              Memories
            </h2>
            <MemoriesSection library={library.id} />
          </section>

          <ul className={styles.cards}>
            <li className={styles.card}>
              <Link className={styles.cardLink} href="/files">
                <span className={styles.cardTitle}>Files</span>
                <span className={styles.cardDetail}>
                  {counts.items === null
                    ? "Browse your library"
                    : `${counts.items} item${counts.items === 1 ? "" : "s"} in the top level`}
                </span>
              </Link>
            </li>
            <li className={styles.card}>
              <Link className={styles.cardLink} href="/photos">
                <span className={styles.cardTitle}>Photos</span>
                <span className={styles.cardDetail}>
                  {counts.photos === null
                    ? "See your pictures"
                    : `${counts.photos} photo${counts.photos === 1 ? "" : "s"} indexed`}
                </span>
              </Link>
            </li>
            <li className={styles.card}>
              <Link className={styles.cardLink} href="/search">
                <span className={styles.cardTitle}>Search</span>
                <span className={styles.cardDetail}>Find something by name</span>
              </Link>
            </li>
            <li className={styles.card}>
              <Link className={styles.cardLink} href="/more">
                <span className={styles.cardTitle}>Library and trash</span>
                <span className={styles.cardDetail}>Scan for new files, restore deletions</span>
              </Link>
            </li>
          </ul>
        </>
      ) : (
        <EmptyState
          title="No library yet"
          description="This account is not a member of any library."
        />
      )}
    </>
  );
}
