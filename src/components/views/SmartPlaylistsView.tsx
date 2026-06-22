// SmartPlaylistsView — grid of smart playlists + inline create form.
// Each card shows name + rule summary + limit.

import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { toast } from "sonner";

import { getSmartPlaylists, createSmartPlaylist, deleteSmartPlaylist, type SmartPlaylist, type CreateSmartPlaylistArgs } from "../../lib/tauri";
import { useServerStore } from "../../stores/serverStore";

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
      setError((e as Error).message ?? String(e));
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
      setForm({ name: "", field: "title", operator: "contains", value: "", sortField: "title", sortDir: "asc", limitN: 50 });
      void load();
    } catch (e) {
      toast.error(`Couldn't create: ${(e as Error).message}`);
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
      toast.error(`Couldn't delete: ${(e as Error).message}`);
    } finally {
      setDeleteId(null);
    }
  };

  if (!activeServerId) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-bg-raised bg-bg-subtle p-6">
        <div className="text-base font-medium text-fg">No server connected</div>
        <p className="text-sm text-fg-subtle">Connect a server in Settings to manage smart playlists.</p>
      </div>
    );
  }

  return (
    <section className="flex flex-col gap-6 p-6">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <h1 className="text-2xl font-semibold">Smart Playlists</h1>
        <span className="text-xs text-fg-muted">Single-rule evaluation — Phase 9</span>
      </header>

      {/* Create form */}
      <div className="flex flex-col gap-4 rounded-md border border-bg-raised bg-bg-subtle p-4">
        <h2 className="text-sm font-medium text-fg">Create new smart playlist</h2>
        <div className="grid grid-cols-[1fr_auto_auto_auto_auto] gap-2 items-end">
          <div className="flex flex-col gap-1">
            <label className="text-xs text-fg-subtle" htmlFor="sp-name">Name</label>
            <input
              id="sp-name"
              type="text"
              value={form.name}
              onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
              placeholder="My smart playlist"
              className="input w-full"
            />
          </div>
          <div className="flex flex-col gap-1">
            <label className="text-xs text-fg-subtle" htmlFor="sp-field">Field</label>
            <select
              id="sp-field"
              value={form.field}
              onChange={(e) => setForm((f) => ({ ...f, field: e.target.value as typeof form.field }))}
              className="input w-full"
            >
              {FIELDS.map((f) => <option key={f.value} value={f.value}>{f.label}</option>)}
            </select>
          </div>
          <div className="flex flex-col gap-1">
            <label className="text-xs text-fg-subtle" htmlFor="sp-op">Operator</label>
            <select
              id="sp-op"
              value={form.operator}
              onChange={(e) => setForm((f) => ({ ...f, operator: e.target.value as typeof f.operator }))}
              className="input w-full"
            >
              {OPERATORS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
            </select>
          </div>
          <div className="flex flex-col gap-1">
            <label className="text-xs text-fg-subtle" htmlFor="sp-value">Value</label>
            <input
              id="sp-value"
              type="text"
              value={form.value}
              onChange={(e) => setForm((f) => ({ ...f, value: e.target.value }))}
              placeholder="search term"
              className="input w-full"
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
        <div className="flex gap-2 items-center text-xs text-fg-subtle">
          <span>Sort by:</span>
          <select
            value={form.sortField}
            onChange={(e) => setForm((f) => ({ ...f, sortField: e.target.value as typeof f.sortField }))}
            className="input py-1 text-xs"
          >
            {SORT_FIELDS.map((f) => <option key={f.value} value={f.value}>{f.label}</option>)}
          </select>
          <select
            value={form.sortDir}
            onChange={(e) => setForm((f) => ({ ...f, sortDir: e.target.value as typeof f.sortDir }))}
            className="input py-1 text-xs"
          >
            {SORT_DIRS.map((d) => <option key={d.value} value={d.value}>{d.label}</option>)}
          </select>
          <span className="ml-2">Limit:</span>
          <select
            value={form.limitN}
            onChange={(e) => setForm((f) => ({ ...f, limitN: Number(e.target.value) }))}
            className="input py-1 text-xs"
          >
            {LIMIT_OPTIONS.map((n) => <option key={n} value={n}>{n}</option>)}
          </select>
          <span>tracks</span>
        </div>
      </div>

      {/* List */}
      {loading ? (
        <p className="text-fg-subtle text-sm" role="status">Loading smart playlists…</p>
      ) : error ? (
        <div className="flex flex-col items-start gap-3 rounded-md border border-red-900 bg-red-950 p-6">
          <div className="text-base font-medium text-red-400">Failed to load</div>
          <p className="text-sm text-red-300">{error}</p>
          <button type="button" onClick={() => void load()} className="btn-ghost text-sm">Retry</button>
        </div>
      ) : playlists.length === 0 ? (
        <div className="flex flex-col items-start gap-3 rounded-md border border-bg-raised bg-bg-subtle p-6">
          <div className="text-base font-medium text-fg">No smart playlists yet</div>
          <p className="text-sm text-fg-subtle">Fill in the form above to create your first one.</p>
        </div>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(20rem,1fr))] gap-4">
          {playlists.map((sp) => (
            <div
              key={sp.id}
              className="flex flex-col gap-2 rounded-md border border-bg-raised bg-bg-subtle p-4 hover:border-accent/50 hover:bg-bg-raised transition-colors"
            >
              <div className="flex items-start justify-between gap-2">
                <Link
                  to={`/smart-playlists/${encodeURIComponent(sp.id)}`}
                  className="text-sm font-medium text-fg hover:text-white truncate"
                >
                  {sp.name}
                </Link>
                <button
                  type="button"
                  onClick={() => setDeleteId(sp.id)}
                  aria-label={`Delete ${sp.name}`}
                  className="shrink-0 rounded-md p-1 text-fg-muted hover:bg-bg-raised hover:text-red-400 focus:outline-none"
                >
                  ✕
                </button>
              </div>
              <div className="text-xs text-fg-subtle">
                {FIELDS.find((f) => f.value === sp.rule.field)?.label ?? sp.rule.field}{" "}
                {OPERATORS.find((o) => o.value === sp.rule.operator)?.label ?? sp.rule.operator}{" "}
                <span className="font-medium text-fg">"{sp.rule.value}"</span>
              </div>
              <div className="flex gap-3 text-xs text-fg-muted">
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
                    className="rounded px-2 py-0.5 bg-bg-raised text-fg hover:text-fg"
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
