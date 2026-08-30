"use client";

import { useId, useState, type FormEvent } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import { useActiveLibrary } from "@/components/session/session-provider";
import { contentUrl, searchLibrary } from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { SearchResult } from "@/lib/api/types";
import { snippetSegments } from "@/lib/api/types";
import { formatBytes, parentOf } from "@/lib/format";
import formStyles from "@/components/ui/form.module.css";
import styles from "./search.module.css";

type Results =
  | { phase: "idle" }
  | { phase: "searching" }
  | { phase: "done"; term: string; items: SearchResult[] }
  | { phase: "failed"; problem: ApiProblem };

export default function SearchPage() {
  const library = useActiveLibrary();
  const inputId = useId();
  const [term, setTerm] = useState("");
  const [results, setResults] = useState<Results>({ phase: "idle" });

  async function submit(event: FormEvent) {
    event.preventDefault();

    const query = term.trim();
    if (!query || !library) {
      return;
    }

    setResults({ phase: "searching" });
    const result = await searchLibrary(library.id, query);

    setResults(
      result.ok
        ? { phase: "done", term: query, items: result.data }
        : { phase: "failed", problem: result.problem },
    );
  }

  return (
    <>
      <h1>Search</h1>

      <form className={styles.form} onSubmit={submit} role="search">
        <label className={formStyles.label} htmlFor={inputId}>
          Search your library
        </label>
        <div className={styles.row}>
          <input
            id={inputId}
            className={formStyles.input}
            type="search"
            value={term}
            onChange={(event) => setTerm(event.target.value)}
            placeholder="File or folder name"
            autoComplete="off"
          />
          <Button type="submit" variant="primary" disabled={!library || term.trim() === ""}>
            Search
          </Button>
        </div>
        <p className={formStyles.hint}>
          Searches file names and the text inside documents. Run a scan from More after
          adding files.
        </p>
      </form>

      {results.phase === "searching" ? <PendingState label="Searching…" /> : null}

      {results.phase === "failed" ? (
        <ErrorState title="The search failed" description={results.problem.detail} />
      ) : null}

      {results.phase === "done" && results.items.length === 0 ? (
        <EmptyState
          title="No matches"
          description={`Nothing in this library matches “${results.term}”.`}
        />
      ) : null}

      {results.phase === "done" && results.items.length > 0 ? (
        <ul className={styles.results}>
          {results.items.map((item) => (
            <li key={item.id} className={styles.result}>
              <span className={styles.resultName}>{item.name}</span>
              <span className={styles.resultMeta}>
                {item.kind === "folder" ? "Folder" : formatBytes(item.sizeBytes)} ·{" "}
                {item.path}
                {item.matched !== "name" ? (
                  <span className={styles.badge}>found in the document</span>
                ) : null}
              </span>
              {item.snippet ? (
                <span className={styles.snippet}>
                  {snippetSegments(item.snippet).map((segment, index) =>
                    segment.matched ? (
                      <mark key={index} className={styles.mark}>
                        {segment.text}
                      </mark>
                    ) : (
                      <span key={index}>{segment.text}</span>
                    ),
                  )}
                </span>
              ) : null}
              <span className={styles.resultActions}>
                <Link
                  className={styles.action}
                  href={
                    item.kind === "folder"
                      ? `/files?path=${encodeURIComponent(item.path)}`
                      : `/files?path=${encodeURIComponent(parentOf(item.path))}`
                  }
                >
                  Show in Files
                </Link>
                {item.kind === "file" ? (
                  <a className={styles.action} href={contentUrl(item.id)} download={item.name}>
                    Download
                  </a>
                ) : null}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
    </>
  );
}
