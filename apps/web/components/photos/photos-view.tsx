"use client";

import { useCallback, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { EmptyState, ErrorState, PendingState } from "@/components/ui/states";
import {
  addFavorite,
  addToAlbum,
  createAlbum,
  deleteAlbum,
  fetchAlbum,
  fetchAlbums,
  fetchFavorites,
  removeFavorite,
  removeFromAlbum,
  renameAlbum,
  thumbnailUrl,
} from "@/lib/api/endpoints";
import type { ApiProblem } from "@/lib/api/problem";
import type { Album, AlbumContents, Item } from "@/lib/api/types";
import { useAsyncData } from "@/lib/hooks/use-async-data";
import { PhotoGrid } from "./photo-grid";
import { PhotoTile } from "./photo-tile";
import { PlacesView } from "./places-view";
import styles from "./photo-grid.module.css";

type View = "timeline" | "albums" | "favorites" | "places";

/**
 * Photos, in the three ways people actually look at them: everything in
 * order, the sets they made themselves, and the ones they starred.
 *
 * Favorites are loaded here rather than inside each view so a star shows
 * as filled the moment it is set, wherever the picture appears.
 */
export function PhotosView({ library }: { library: string }) {
  const [view, setView] = useState<View>("timeline");
  const [openAlbum, setOpenAlbum] = useState<string | null>(null);

  const loadFavorites = useCallback(
    (signal: AbortSignal) => fetchFavorites(library, { signal }),
    [library],
  );
  const { state: favoriteState, reload: reloadFavorites } = useAsyncData<Item[]>(loadFavorites);

  const favorites = useMemo(
    () =>
      new Set(
        favoriteState.phase === "ready" ? favoriteState.data.map((item) => item.id) : [],
      ),
    [favoriteState],
  );

  const onToggleFavorite = useCallback(
    async (photo: Item) => {
      const starred = favorites.has(photo.id);
      const result = starred ? await removeFavorite(photo.id) : await addFavorite(photo.id);

      if (result.ok) {
        await reloadFavorites();
      }
    },
    [favorites, reloadFavorites],
  );

  if (openAlbum) {
    return (
      <AlbumView
        album={openAlbum}
        favorites={favorites}
        onToggleFavorite={onToggleFavorite}
        onClose={() => setOpenAlbum(null)}
      />
    );
  }

  return (
    <>
      <div className={styles.views} role="tablist" aria-label="Photo views">
        {(["timeline", "albums", "favorites", "places"] as const).map((candidate) => (
          <Button
            key={candidate}
            role="tab"
            aria-selected={view === candidate ? "true" : "false"}
            variant={view === candidate ? "primary" : "quiet"}
            onClick={() => setView(candidate)}
          >
            {candidate === "timeline"
              ? "Timeline"
              : candidate === "albums"
                ? "Albums"
                : candidate === "favorites"
                  ? "Favorites"
                  : "Places"}
          </Button>
        ))}
      </div>

      {view === "timeline" ? (
        <TimelineView
          library={library}
          favorites={favorites}
          onToggleFavorite={onToggleFavorite}
        />
      ) : null}

      {view === "albums" ? <AlbumsView library={library} onOpen={setOpenAlbum} /> : null}

      {view === "places" ? <PlacesView library={library} /> : null}

      {view === "favorites" ? (
        <FavoritesView
          state={favoriteState}
          favorites={favorites}
          onToggleFavorite={onToggleFavorite}
        />
      ) : null}
    </>
  );
}

/**
 * The timeline, with a selection mode for putting pictures into an
 * album. Selection is a mode rather than always-on because the ordinary
 * thing to do with a photo is open it, not tick it.
 */
function TimelineView({
  library,
  favorites,
  onToggleFavorite,
}: {
  library: string;
  favorites: ReadonlySet<string>;
  onToggleFavorite: (photo: Item) => void;
}) {
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [notice, setNotice] = useState<string | null>(null);
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const loadAlbums = useCallback(
    (signal: AbortSignal) => fetchAlbums(library, { signal }),
    [library],
  );
  const { state: albumState, reload: reloadAlbums } = useAsyncData<Album[]>(loadAlbums);
  const albums = albumState.phase === "ready" ? albumState.data : [];

  function onToggleSelected(photo: Item) {
    setSelected((current) => {
      const next = new Set(current);
      if (!next.delete(photo.id)) {
        next.add(photo.id);
      }

      return next;
    });
  }

  async function addSelectionTo(album: string, name: string) {
    setProblem(null);

    const result = await addToAlbum(album, [...selected]);

    if (!result.ok) {
      setProblem(result.problem);
      return;
    }

    setNotice(`Added ${selected.size} to “${name}”.`);
    setSelected(new Set());
    setSelecting(false);
    await reloadAlbums();
  }

  async function addSelectionToNewAlbum() {
    const name = window.prompt("Name for the new album");
    if (!name?.trim()) {
      return;
    }

    const created = await createAlbum(library, name.trim());
    if (!created.ok) {
      setProblem(created.problem);
      return;
    }

    await addSelectionTo(created.data.id, created.data.name);
  }

  return (
    <>
      <div className={styles.bar}>
        {selecting ? (
          <>
            <span className={styles.barNote}>
              {selected.size === 0
                ? "Choose photos to add to an album."
                : `${selected.size} selected`}
            </span>
            {albums.map((album) => (
              <Button
                key={album.id}
                disabled={selected.size === 0}
                onClick={() => void addSelectionTo(album.id, album.name)}
              >
                Add to {album.name}
              </Button>
            ))}
            <Button
              variant="primary"
              disabled={selected.size === 0}
              onClick={() => void addSelectionToNewAlbum()}
            >
              Add to a new album
            </Button>
            <Button
              variant="quiet"
              onClick={() => {
                setSelecting(false);
                setSelected(new Set());
              }}
            >
              Cancel
            </Button>
          </>
        ) : (
          <Button onClick={() => setSelecting(true)}>Select photos</Button>
        )}
      </div>

      {notice ? (
        <p className={styles.barNote} role="status">
          {notice}
        </p>
      ) : null}
      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}

      <PhotoGrid
        library={library}
        selecting={selecting}
        selected={selected}
        favorites={favorites}
        onToggleSelected={onToggleSelected}
        onToggleFavorite={onToggleFavorite}
      />
    </>
  );
}

/** The albums in a library, as covers. */
function AlbumsView({
  library,
  onOpen,
}: {
  library: string;
  onOpen: (album: string) => void;
}) {
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchAlbums(library, { signal }),
    [library],
  );
  const { state, reload } = useAsyncData<Album[]>(load);

  async function onCreate() {
    const name = window.prompt("Name for the new album");
    if (!name?.trim()) {
      return;
    }

    const created = await createAlbum(library, name.trim());
    if (created.ok) {
      await reload();
    } else {
      setProblem(created.problem);
    }
  }

  if (state.phase === "loading") {
    return <PendingState label="Loading albums…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="Albums could not be loaded"
        description={state.problem.detail}
        actionLabel="Try again"
        onAction={() => void reload()}
      />
    );
  }

  return (
    <>
      <div className={styles.bar}>
        <Button variant="primary" onClick={() => void onCreate()}>
          New album
        </Button>
      </div>

      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}

      {state.data.length === 0 ? (
        <EmptyState
          title="No albums yet"
          description="An album is a set of pictures you arrange yourself. Make one here, then add photos from the timeline."
        />
      ) : (
        <ul className={styles.albums}>
          {state.data.map((album) => (
            <li key={album.id}>
              <button
                type="button"
                className={styles.albumCard}
                onClick={() => onOpen(album.id)}
              >
                {album.coverItemId ? (
                  /* The optimizer cannot reach a private,
                     session-protected origin. */
                  // eslint-disable-next-line @next/next/no-img-element
                  <img
                    className={styles.albumCover}
                    src={thumbnailUrl(album.coverItemId, "medium")}
                    alt=""
                    loading="lazy"
                    decoding="async"
                  />
                ) : (
                  <span className={`${styles.albumCover} ${styles.albumEmptyCover}`}>Empty</span>
                )}
                <span className={styles.albumName}>{album.name}</span>
                <span className={styles.albumCount}>
                  {album.itemCount} {album.itemCount === 1 ? "photo" : "photos"}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

/** One album: its pictures in the order they were arranged. */
function AlbumView({
  album,
  favorites,
  onToggleFavorite,
  onClose,
}: {
  album: string;
  favorites: ReadonlySet<string>;
  onToggleFavorite: (photo: Item) => void;
  onClose: () => void;
}) {
  const [problem, setProblem] = useState<ApiProblem | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => fetchAlbum(album, { signal }),
    [album],
  );
  const { state, reload } = useAsyncData<AlbumContents>(load);

  async function onRename(current: string) {
    const name = window.prompt("Rename this album", current);
    if (!name?.trim() || name.trim() === current) {
      return;
    }

    const result = await renameAlbum(album, name.trim());
    if (result.ok) {
      await reload();
    } else {
      setProblem(result.problem);
    }
  }

  async function onDelete(name: string) {
    if (!window.confirm(`Delete the album “${name}”? The photos in it are kept.`)) {
      return;
    }

    const result = await deleteAlbum(album);
    if (result.ok) {
      onClose();
    } else {
      setProblem(result.problem);
    }
  }

  async function onRemove(photo: Item) {
    const result = await removeFromAlbum(album, photo.id);
    if (result.ok) {
      await reload();
    } else {
      setProblem(result.problem);
    }
  }

  if (state.phase === "loading") {
    return <PendingState label="Loading album…" />;
  }

  if (state.phase === "failed") {
    return (
      <ErrorState
        title="This album could not be opened"
        description={state.problem.detail}
        actionLabel="Back to albums"
        onAction={onClose}
      />
    );
  }

  const { album: details, items } = state.data;

  return (
    <>
      <div className={styles.bar}>
        <Button variant="quiet" onClick={onClose}>
          ← All albums
        </Button>
        <span className={styles.barNote}>
          {details.name} · {items.length} {items.length === 1 ? "photo" : "photos"}
        </span>
        <Button onClick={() => void onRename(details.name)}>Rename</Button>
        <Button variant="quiet" onClick={() => void onDelete(details.name)}>
          Delete album
        </Button>
      </div>

      {problem ? <ErrorState title="That did not work" description={problem.detail} /> : null}

      {items.length === 0 ? (
        <EmptyState
          title="This album is empty"
          description="Add photos to it from the timeline: choose Select photos, pick some, then add them here."
        />
      ) : (
        <ul className={styles.grid}>
          {items.map((photo) => (
            <PhotoTile
              key={photo.id}
              photo={photo}
              favorite={favorites.has(photo.id)}
              onToggleFavorite={onToggleFavorite}
              action={{ label: "Remove from this album:", onAction: (item) => void onRemove(item) }}
            />
          ))}
        </ul>
      )}
    </>
  );
}

/** The pictures this person starred. */
function FavoritesView({
  state,
  favorites,
  onToggleFavorite,
}: {
  state: ReturnType<typeof useAsyncData<Item[]>>["state"];
  favorites: ReadonlySet<string>;
  onToggleFavorite: (photo: Item) => void;
}) {
  if (state.phase === "loading") {
    return <PendingState label="Loading favorites…" />;
  }

  if (state.phase === "failed") {
    return <ErrorState title="Favorites could not be loaded" description={state.problem.detail} />;
  }

  if (state.data.length === 0) {
    return (
      <EmptyState
        title="Nothing starred yet"
        description="Star a photo in the timeline and it appears here. Favorites are yours: other people in this library have their own."
      />
    );
  }

  return (
    <ul className={styles.grid}>
      {state.data.map((photo) => (
        <PhotoTile
          key={photo.id}
          photo={photo}
          favorite={favorites.has(photo.id)}
          onToggleFavorite={onToggleFavorite}
        />
      ))}
    </ul>
  );
}
