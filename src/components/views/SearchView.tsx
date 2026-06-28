// Phase 0 placeholder. Real implementation in Phase 8 (FTS5 search).

import { type FormEvent, useState } from "react";
import { search } from "@/lib/tauri";
import type { SearchResults } from "@/types/domain";

export function SearchView() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResults | null>(null);

  const onSubmit = async (event: FormEvent) => {
    event.preventDefault();
    try {
      const r = await search(query);
      setResults(r);
    } catch (err) {
      console.error("search failed", err);
    }
  };

  return (
    <section className="p-6">
      <h1 className="mb-4 text-2xl font-semibold">Search</h1>
      <form onSubmit={onSubmit} className="mb-4 flex gap-2">
        <input
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
          placeholder="Albums, tracks, artists…"
          className="flex-1 rounded-md border border-border bg-muted px-3 py-2 text-sm text-foreground placeholder:text-muted focus:border-primary focus:outline-none"
        />
        <button type="submit" className="btn-primary">
          Search
        </button>
      </form>
      {results && (
        <div className="space-y-2 text-sm text-muted-foreground">
          {results.albums.length + results.tracks.length + results.artists.length === 0
            ? "No results"
            : `${results.albums.length} albums, ${results.tracks.length} tracks, ${results.artists.length} artists`}
        </div>
      )}
    </section>
  );
}
