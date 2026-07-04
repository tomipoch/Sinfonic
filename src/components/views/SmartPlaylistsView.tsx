// SmartPlaylistsView — grid of smart playlists + inline create form.
// Each card shows name + rule summary + limit.

import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { toast } from "sonner";
import { extractError } from "@/lib/errors";
import {
  type CreateSmartPlaylistArgs,
  createSmartPlaylist,
  deleteSmartPlaylist,
  getSmartPlaylists,
  type SmartPlaylist,
} from "@/lib/tauri";
import { useServerStore } from "@/stores/serverStore";

const FIELDS = [
  { value: "title", label: "Title" },
  { value: "artist", label: "Artist" },
  { value: "album", label: "Album" },
  { value: "genre", label: "Genre" },
  { value: "year", label: "Year" },
  { value: "favorite", label: "Favorite" },
  { value: "duration_seconds", label: "Duration (sec)" },
  { value: "track_number", label: "Track #" },
] as const;

const OPERATORS = [
  { value: "contains", label: "contains" },
  { value: "starts_with", label: "starts with" },
  { value: "ends_with", label: "ends with" },
  { value: "equals", label: "equals" },
  { value: "less_than", label: "<" },
  { value: "greater_than", label: ">" },
  { value: "not_contains", label: "doesn't contain" },
  { value: "not_equals", label: "≠" },
] as const;

const SORT_FIELDS = [
  { value: "title", label: "Title" },
  { value: "artist", label: "Artist" },
  { value: "album", label: "Album" },
  { value: "year", label: "Year" },
  { value: "duration_seconds", label: "Duration" },
  { value: "random", label: "Random" },
  { value: "date_added", label: "Date Added" },
] as const;

const SORT_DIRS = [
  { value: "asc", label: "↑ Asc" },
  { value: "desc", label: "↓ Desc" },
] as const;

const LIMIT_OPTIONS = [10, 25, 50, 100, 200] as const;

export function SmartPlaylistsView() {
  const activeServerId = useServerStore((s) => s.activeServerId);

  const [playlists, setPlaylists] = useState<SmartPlaylist[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);

  // Create form state
  const [form, setForm] = useState<CreateSmartPlaylistArgs>({
    name: "",
    field: "title",
    operator: "contains",
    value: "",
    sortField: "title",
    sortDir: "asc",
    limitN: 50,
  });

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await getSmartPlaylists();
      setPlaylists(result);
    } catch (e) {
      setError(extractError(e, "couldn't load smart playlists"));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (activeServerId) void load();
  }, [activeServerId]);

  const handleCreate = async () => {
    if (!form.name.trim()) {
      toast.error("Name is required");
      return;
    }
    if (!form.value.trim()) {
      toast.error("Value is required");
      return;
    }
    setCreating(true);
    try {
      await createSmartPlaylist(form);
      toast.success("Smart playlist created");
      setForm({
        name: "",
        field: "title",
        operator: "contains",
        value: "",
        sortField: "title",
        sortDir: "asc",
        limitN: 50,
      });
      void load();
    } catch (e) {
      toast.error(`Couldn't create: ${extractError(e, "unknown error")}`);
    } finally {
      setCreating(false);
    }
  };

  const handleDelete = async (spId: string) => {
    try {
      await deleteSmartPlaylist(spId);
      toast.success("Deleted");
      void load();
    } catch (e) {
      toast.error(`Couldn't delete: ${extractError(e, "unknown error")}`);
    } finally {
      setDeleteId(null);
    }
  };

  if (!activeServerId) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">No server connected</div>
        <p className="text-sm text-muted-foreground">
          Connect a server in Settings to manage smart playlists.
        </p>
      </div>
    );
  }

  return (
    <section className="flex flex-col gap-6 p-6">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <h1 className="text-2xl font-semibold">Smart Playlists</h1>
        <span className="text-xs text-muted">Single-rule evaluation — Phase 9</span>
      </header>

      {/* Create form */}
      <div className="flex flex-col gap-5 rounded-md border border-border bg-card p-5">
        <h2 className="text-sm font-medium text-foreground">Create new smart playlist</h2>

        {/* Row 1 — name + create button */}
        <div className="grid grid-cols-1 items-end gap-3 sm:grid-cols-[1fr_auto]">
          <div>
            <label className="label" htmlFor="sp-name">
              Name
            </label>
            <input
              id="sp-name"
              type="text"
              value={form.name}
              onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
              placeholder="My smart playlist"
              className="input"
            />
          </div>
          <button
            type="button"
            onClick={() => void handleCreate()}
            disabled={creating}
            className="btn-primary"
          >
            {creating ? "Creating…" : "Create"}
          </button>
        </div>

        {/* Row 2 — rule (field/operator/value) */}
        <div>
          <div className="label">Rule</div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-[1fr_1fr_1fr]">
            <select
              aria-label="Field"
              value={form.field}
              onChange={(e) => setForm((f) => ({ ...f, field: e.target.value as typeof f.field }))}
              className="select"
            >
              {FIELDS.map((f) => (
                <option key={f.value} value={f.value}>
                  {f.label}
                </option>
              ))}
            </select>
            <select
              aria-label="Operator"
              value={form.operator}
              onChange={(e) =>
                setForm((f) => ({ ...f, operator: e.target.value as typeof f.operator }))
              }
              className="select"
            >
              {OPERATORS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
            <input
              aria-label="Value"
              type="text"
              value={form.value}
              onChange={(e) => setForm((f) => ({ ...f, value: e.target.value }))}
              placeholder="search term"
              className="input"
            />
          </div>
        </div>

        {/* Row 3 — sort + limit */}
        <div>
          <div className="label">Sort &amp; limit</div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-[1fr_1fr_1fr]">
            <select
              aria-label="Sort field"
              value={form.sortField}
              onChange={(e) =>
                setForm((f) => ({
                  ...f,
                  sortField: e.target.value as typeof f.sortField,
                }))
              }
              className="select"
            >
              {SORT_FIELDS.map((f) => (
                <option key={f.value} value={f.value}>
                  {f.label}
                </option>
              ))}
            </select>
            <select
              aria-label="Sort direction"
              value={form.sortDir}
              onChange={(e) =>
                setForm((f) => ({
                  ...f,
                  sortDir: e.target.value as typeof f.sortDir,
                }))
              }
              className="select"
            >
              {SORT_DIRS.map((d) => (
                <option key={d.value} value={d.value}>
                  {d.label}
                </option>
              ))}
            </select>
            <select
              aria-label="Limit"
              value={form.limitN}
              onChange={(e) => setForm((f) => ({ ...f, limitN: Number(e.target.value) }))}
              className="select"
            >
              {LIMIT_OPTIONS.map((n) => (
                <option key={n} value={n}>
                  {n} tracks
                </option>
              ))}
            </select>
          </div>
        </div>
      </div>

      {/* List */}
      {loading ? (
        <p className="text-muted-foreground text-sm" role="status">
          Loading smart playlists…
        </p>
      ) : error ? (
        <div className="flex flex-col items-start gap-3 rounded-md border border-red-900 bg-red-950 p-6">
          <div className="text-base font-medium text-red-400">Failed to load</div>
          <p className="text-sm text-red-300">{error}</p>
          <button type="button" onClick={() => void load()} className="btn-ghost text-sm">
            Retry
          </button>
        </div>
      ) : playlists.length === 0 ? (
        <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
          <div className="text-base font-medium text-foreground">No smart playlists yet</div>
          <p className="text-sm text-muted-foreground">
            Fill in the form above to create your first one.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(20rem,1fr))] gap-4">
          {playlists.map((sp) => (
            <div
              key={sp.id}
              className="flex flex-col gap-2 rounded-md border border-border bg-muted p-4 hover:border-primary/50 hover:bg-card transition-colors"
            >
              <div className="flex items-start justify-between gap-2">
                <Link
                  to={`/smart-playlists/${encodeURIComponent(sp.id)}`}
                  className="text-sm font-medium text-foreground hover:text-white truncate"
                >
                  {sp.name}
                </Link>
                <button
                  type="button"
                  onClick={() => setDeleteId(sp.id)}
                  aria-label={`Delete ${sp.name}`}
                  className="shrink-0 rounded-md p-1 text-muted hover:bg-card hover:text-red-400 focus:outline-none"
                >
                  ✕
                </button>
              </div>
              <div className="text-xs text-muted-foreground">
                {FIELDS.find((f) => f.value === sp.rule.field)?.label ?? sp.rule.field}{" "}
                {OPERATORS.find((o) => o.value === sp.rule.operator)?.label ?? sp.rule.operator}{" "}
                <span className="font-medium text-foreground">"{sp.rule.value}"</span>
              </div>
              <div className="flex gap-3 text-xs text-muted">
                <span>Sort: {SORT_FIELDS.find((f) => f.value === sp.sortField)?.label}</span>
                <span>· Limit: {sp.limitN}</span>
              </div>

              {deleteId === sp.id && (
                <div className="flex items-center gap-2 rounded-md bg-red-950 p-2 text-xs">
                  <span className="text-red-300">Delete "{sp.name}"?</span>
                  <button
                    type="button"
                    onClick={() => void handleDelete(sp.id)}
                    className="rounded px-2 py-0.5 bg-red-700 text-white hover:bg-red-600"
                  >
                    Yes
                  </button>
                  <button
                    type="button"
                    onClick={() => setDeleteId(null)}
                    className="rounded px-2 py-0.5 bg-card text-foreground hover:text-foreground"
                  >
                    No
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
