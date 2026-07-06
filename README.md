# Sinfonic

A cross-platform desktop music player for self-hosted media servers, written in Rust (Tauri v2) + React 19.

## Supported sources

- **Jellyfin** — full library sync, playback, now-playing reporting
- **Subsonic / Navidrome / Airsonic** — album-tracks fan-out sync, signed stream URLs, scrobble forwarding
- **Local files** — scan a music folder, dedup by content-derived IDs, embedded cover art

## Layout

```
.
├─ src/                    # React 19 + Vite frontend
├─ src-tauri/              # Rust backend
│  ├─ src/                 # App crate, IPC commands, AppState
│  ├─ crates/
│  │  ├─ domain/           # Pure-data types (Albums, Tracks, Queue…)
│  │  ├─ source/           # `MusicProvider` trait + shared DTOs
│  │  ├─ source-jellyfin/  # Jellyfin implementation
│  │  ├─ source-subsonic/  # Subsonic/Navidrome implementation
│  │  ├─ source-local/     # Local files implementation
│  │  ├─ playback/         # rodio-based audio engine + EQ
│  │  ├─ library/          # SQLite cache + album-art hash-dedup
│  │  ├─ secrets/          # OS keyring wrapper
│  │  ├─ lastfm/           # Last.fm scrobble client
│  │  └─ lyrics/           # LRCLIB fallback
│  ├─ tests/               # End-to-end integration tests
│  ├─ capabilities/        # Per-window Tauri permission JSON
│  └─ tauri.conf.json
├─ AGENTS.md              # Contribution guide
├─ ROADMAP.md             # Phase-by-phase history
└─ PLAN.md                # UI redesign plan
```

## Prerequisites

- **Node 20+** + `pnpm` (install via `npm i -g pnpm`)
- **Rust stable** (toolchain in `rust-toolchain.toml`)
- **Tauri v2** system deps per platform:
  - macOS: Xcode Command Line Tools
  - Windows: WebView2 runtime + MSVC build tools
  - Linux: `webkit2gtk-4.1`, `libayatana-appindicator3`, `librsvg2`

## Commands

```bash
pnpm tauri dev      # Full app — runs Vite + Rust in dev mode
pnpm tauri build    # Production bundle (.app / .msi / .AppImage)
pnpm dev            # Vite only — http://localhost:1420
pnpm build          # tsc + vite build
pnpm test           # Vitest (frontend) — 117 tests
pnpm ci:biome       # Biome format + lint check

cd src-tauri
cargo check         # Workspace typecheck
cargo test          # Workspace tests — 274 tests across 10 crates
cargo clippy --workspace --all-targets
```

## Architecture

- **Two windows**: `main` (BrowserRouter, full UI) and `settings` (crossfade, EQ, theme, server list). Each has its own capability JSON in `src-tauri/capabilities/`.
- **Provider-agnostic UI**: the frontend calls `provider_list_*` Tauri commands; Subsonic takes the SQLite-cached path (auto-warmed by `kick_*_background_sync`), Jellyfin/Local hit the upstream directly.
- **Album-art hash-dedup**: `AlbumArtCache` keys by `(provider, item_id, tag)` and aliases entries with byte-identical content so tracks/albums/artists that share an image in Subsonic use one file on disk.
- **Scrobble watcher**: 1 Hz background task that forwards to BOTH Last.fm (when authenticated) and the active music provider's native scrobble endpoint (`/rest/scrobble` for Subsonic, `/Sessions/Playing` for Jellyfin).

## License

[Private — not yet licensed for redistribution.]
