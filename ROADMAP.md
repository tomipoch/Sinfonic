# Sinfonic — Roadmap

## ✅ Completado

### Fase 0 — Foundation
- Tauri v2 + React 19 + TypeScript + Vite + pnpm
- Workspace Rust con 8 crates (`domain`, `source`, `source-jellyfin`, `source-subsonic`, `source-local`, `playback`, `library`, `secrets`)
- `MusicProvider` trait con 30+ métodos y 18 capabilities
- Skeleton de comandos Tauri + payloads + eventos
- Layout React (sidebar + player bar + empty views), Tailwind, React Router, Zustand, sonner

### Fase 1 — Domain + Queue + Playback
- `QueueEngine` con mutaciones completas, navegación, shuffle, repeat
- `PlaybackState` mirror in-memory
- Comandos `play_track`, `pause`, `resume`, `stop`, `seek`, `set_volume`, `set_muted`, queue CRUD
- Eventos `queue-changed`, `track-changed`, `playback-state-changed`
- 34 tests de dominio + 5 tests de integración AppState

### Fase 2 — Library (SQLite + FTS5)
- Cache SQL con `rusqlite` (bundled) + `r2d2`
- Migraciones server-scoped (idempotentes)
- Índice FTS5 sobre tracks
- CRUD sobre la librería
- 28 tests unit + 5 tests de integración

### Fase 3 — Jellyfin provider
- Cliente HTTP (auth X-MediaBrowser-Token, discovery, ping, user/library views)
- `MusicProvider` completo: albums, artists, tracks, search, stream URL, scrobble
- Comandos `jellyfin_login` / `jellyfin_logout` / `jellyfin_sync` con persistencia en Keyring
- `SettingsView` UI con discovery, login form y botón de sync
- 7 tests unit (cliente HTTP + provider) + 12 tests de integración (wiremock)

### Fase 4 — Playback engine
- `AudioPlayer` rodio 0.20 (Sink + OutputStream + Symphonia: FLAC/MP3/MP4/AAC/Vorbis/Opus/WAV)
- Resolución de stream URI local / http(s) → `Decoder`
- Position poller cada 250 ms que dispara `PlayerEvent::StateChanged` y `TrackEnded`
- EQ gráfico de 10 bandas (60 Hz … 16 kHz) con biquads RBJ peaking, cascada por canal
- Comandos `set_eq_band`, `reset_eq` + eventos `eq-changed` / `eq-reset` / `track-ended`
- Fallback silencioso para entornos sin dispositivo de audio (CI)
- 14 tests de playback (biquad + eq + stream + player headless)

### Fase 5 — Subsonic / Navidrome provider
- `sinfonic-source-subsonic`: `MusicProvider` completo (albums, artists, tracks, search, playlists, stream, scrobble, lyrics)
- Auth salt+md5 firmada en cada request (`SubsonicSession::sign()` regenera salt+token por llamada)
- Mapeo Subsonic DTO → `sinfonic_domain::*` (prefijo `track-`/`album-`/`artist-`/`playlist-`)
- `AppState.provider: Arc<Mutex<Option<Arc<dyn MusicProvider>>>>` con switch transparente Jellyfin ↔ Subsonic
- Comandos `subsonic_login` + `provider_logout/sync_library/servers/active_server` (renombrados desde `jellyfin_*`)
- `SettingsView`: toggle Jellyfin/Subsonic con placeholders distintos (8096 vs 4533) + helper text + sección de discovery sólo para Jellyfin
- `serverStore`: `LoginRequest` discriminated union por `kind`; dispatch a `jellyfinLogin`/`subsonicLogin`
- 7 tests unit + 14 tests integration con `wiremock`

### Fase 6 — UI real
- `libraryStore`: `loadAlbums/loadArtists/loadTracks/loadAll/reset` + `loading/loaded/error` flags
- `useLibraryAutoLoad` (auto-fetch al cambiar `activeServerId`) + `usePlaybackEvents` (bridge global de eventos)
- `AlbumCover`: gradiente determinista por djb2(album.id) (placeholder temporal)
- `AlbumsTab`: grid con cover + título + artista + año + contador + CTA "Sync library" cuando vacío
- `AlbumDetailView`: header + botón Play-album + tabla de tracks con play por fila (backend: `get_album_detail` + `play_album`)
- `ArtistsTab` + `TracksTab` (sortable) + `ArtistDetailView` (grid filtrado por `artistId` desde caché, sin backend)
- `PlayerBar`: transport (prev/play-pause/next) + seek bar (commits on pointer/key release) + mute + volume slider
- `QueueView`: lista con current-entry highlight + jump-to + remove + repeat cycle (off→all→one) + shuffle + clear

### Fase 7 — Album art + scrobble polish + EQ UI
- **Album art cache** (`sinfonic-library`): `AlbumArtCache` filesystem LRU bajo `app_data_dir/album_art/`, clave SHA-256(provider + image_id + tag) truncada a 32 hex, layout shard (root/ab/abcd….bin + .mime + .meta), `evict_if_over(max_bytes)` por timestamp ascendente, atomic temp+rename
- **`provider_image_bytes` command**: read-through (cache miss → `MusicProvider::image_bytes(ImageRequest{ kind: Primary, size: 600 })` → sniff JPEG/PNG/GIF/WebP magic bytes si falta `Content-Type` → write-through), devuelve `AlbumArtResponse { bytes, content_type, cached }`
- **`ImageBytes.content_type`**: `JellyfinClient::get_bytes` y `SubsonicClient::get_bytes` ahora devuelven `(Vec<u8>, Option<String>)` parseando el header `Content-Type` (charset stripped)
- **`AlbumCover` real**: `<img>` con `URL.createObjectURL(new Blob([Uint8Array], { type: contentType }))`, `URL.revokeObjectURL` en cleanup, `loading="lazy"`, fallback al gradiente en error
- **`useAlbumArtPrewarm`**: dispara `providerImageBytes` para los primeros 24 álbumes al cargar la librería (cache hits son no-ops)
- **`sinfonic-lastfm` crate** (nuevo, 9º workspace member): `LastFmClient` (authenticate + resume + now_playing + scrobble), `signature::sign` (lex sort + concat + md5(api_secret suffix)) verificado contra el ejemplo de la docu de Last.fm, mapeo de error codes (4/9/14 → Auth, 29 → RateLimited, etc.), `ScrobbleSource { User, NonPersonalised, Recommended }`
- **Credenciales en keyring**: `SecretKey::LastFmApiSecret` (JSON `{api_key, api_secret}`) + `LastFmSession` (session key) — variantes del enum que ya existían sin uso; password NUNCA persistido (md5-hash local antes de enviar)
- **`ScrobbleWatcher`**: task en background (1 s tick) que polla `AudioPlayer::cached_state()` + `QueueEngine::current()` → `now_playing` al cambiar track, `scrobble` al cruzar 50 % del duration (una vez por track, dedupe por `HashSet<TrackId>`)
- **Settings UI**: sección Last.fm al final (status pill, form api_key + api_secret + username + password, Connect/Disconnect)
- **EQ panel**: comando `get_eq_bands` (lee `AudioPlayer::eq_bands()`), `useEqStore` Zustand, `EqPanel` con 10 sliders verticales (±12 dB, commits on pointer/key release, subscribe a `eq-changed` / `eq-reset`), botón "EQ" en `PlayerBar` que abre popover anchored arriba a la derecha, Esc cierra

### Fase 8 — Local-files provider
- **`sinfonic-source-local` real impl**: `lofty` 0.21 (default features: MP3/FLAC/OGG/Opus/MP4/WAV) + `walkdir` 2 para scan recursivo
- **`scanner::scan(root) -> ScanResult`**: walkdir → lofty parse → `Vec<Track>` con metadatos (artist/title/album/duration/track#/disc#/year), dedupe de albums/artists via `aggregate_albums` / `aggregate_artists`, embedded art extraction, per-file errors no abortan
- **IDs estables**: `album-<sha256(lower(artist) + "\0" + lower(album))[:16]>`, `artist-<sha256(lower(artist))[:16]>`, `track-<percent-encoded-relative-path>` (rescans no duplican; case-insensitive dedupe)
- **`LocalProvider` impl `MusicProvider`** (~500 LOC): identity/capabilities (`music_folders: true`, otros mínimos), `albums/artists/tracks` paginados desde scan en memoria, `album_detail/artist_detail/track` lookups, `stream` → `StreamDescriptor { uri: "file://…", redacted_uri: <abs path> }`, `image_bytes` → art embebido del primer track con portada, `search` substring case-insensitive, `path_for_track` con `canonicalize` + `strip_prefix` para evitar escapes del music root
- **Capabilities declarados explícitamente**: albums/tracks/artists/album_artists/search/image_metadata/music_folders = true; resto false
- **Persistencia**: filas en `servers` (kind=`local`, base_url=path), `library.replace_albums/artists/tracks` con `server_id="server-local"` — mismas tablas que Jellyfin/Subsonic, cero cambios al SQLite schema
- **Sin cambios en playback**: `playback/src/stream.rs::open_local` ya acepta `file://` URIs
- **`local_login(path)` + `local_rescan` commands**: validan path, construyen `LocalProvider`, llaman `rescan()` (síncrono), escriben SQLite cache, upsertan server row, instalan provider activo
- **Settings UI**: nueva sección "Local files" entre Saved servers y Last.fm — text input del path + Scan/Rescan/Disconnect (scan stats muestran tracks/albums/artists/errors)
- **`ServerKind = "jellyfin" | "subsonic" | "local"`** + dispatch en `serverStore.login` con `LocalLoginRequest { path }`
- **17 source-local tests**: 7 scanner unit + 3 LocalProvider unit + 7 integration con WAV fixtures generados por `hound`

### Fase 9 — Cierre (Playlists + Favoritos + Smart Playlists + UX)
- **Playlist CRUD**: `create_playlist/rename_playlist/delete_playlist/add_playlist_tracks/remove_playlist_entries/move_playlist_entry` + Tauri commands + TS wrappers
- **`PlaylistsView` + `PlaylistDetailView`**: grid de playlists, formulario inline de creación, vista de detalle con lista de tracks + acciones (play/rename/delete)
- **`QueueEngine::add_many()` + `queue_play_next_many()`**: bulk append/insert para DnD
- **Favoritos locales**: `set_track_favorite/set_album_favorite/set_artist_favorite/get_favorites` (local SQLite cache, sin sync al provider)
- **`FavoritesView`**: tabs tracks/albums/artists con inline favorite toggle
- **`FavoriteButton`**: componente reutilizable heart toggle
- **Drag-and-drop**: `useDropTarget` hook, `useKeyboardShortcuts` hook, `queueDnD.ts` helpers; tracks draggable en AlbumsTab/AlbumDetailView/TracksTab/PlaylistDetailView/FavoritesView; drop target en PlayerBar (append)
- **Keyboard shortcuts**: `space` play/pause, `←/→` prev/next, `↑/↓` volume ±5%, `m` mute — ignorando inputs/contentEditable/modifiers
- **Schema v2**: `ALTER TABLE tracks ADD year`, `smart_playlists` table (field/operator/value/sort_field/sort_dir/limit_n)
- **Smart playlists rule engine** (`library/src/smart_playlists.rs`): SQL parametrized WHERE + ORDER BY + LIMIT evaluation, single-rule only
- **Domain entities**: `SmartPlaylist`, `SmartPlaylistRule`, enums `SmartPlaylistRuleField/Operator`, `SmartPlaylistSortField/Direction`
- **Smart playlist Tauri commands**: `get_smart_playlists/create_smart_playlist/delete_smart_playlist/evaluate_smart_playlist`
- **SmartPlaylistsView + SmartPlaylistDetailView**: formulario de creación con field/operator/value/sort/limit, grid de cards, detalle con tracks evaluados

**Total tests: 7** (lib) + **5** (library integration) · clippy clean · pnpm build clean

---

**Estado actual:**
- `feature/fase-9-cierre` HEAD: listo para merge a `develop`
- clippy clean · pnpm build clean
### Fase 10 — UI redesign
- **feature/ui-redesign** (merged `553b8fd`, 2026-06-27): TitleBar + WindowControls consolidation, redesign of Sidebar / TopBar / QueuePanel / AlbumCover to drop the Tauri template chrome and match the macOS / Windows 11 reference shape, `useDropTarget` for drag-to-queue from Album cards, hover-revealed volume + EQ + favorite controls on the PlayerBar.

### Fase 11 — rust-hardening + crossfade + queue snapshots
- **feature/rust-hardening** (merged `453d5e1`): tightened error paths, dropped `unwrap()`s in favour of `?`-propagation in `commands::play_*`, audit-and-fix pass on `RwLock` vs `Mutex` choices in `AppState`.
- **fix/playback-queue-subsonic** (merged `03d9e62`): added `crossfade_enabled` / `crossfade_seconds` prefs persisted via `library_meta`, `QueueEngine::save_snapshot` + `load_snapshot` on the `queue_snapshots` table scoped by `server_id` (schema v6), `AudioPlayer::preload_next` for crossfade ramps, `play_track_with_context` / `play_album_with_context` flows, provider-aware queue restore.

### Fase 12 — direct-fetch-providers (feature/direct-fetch-providers)
- **Subsonic background sync**: provider-direct reads for albums/artists/tracks via fan-out from `getAlbumList2` to `getAlbum`, capped concurrency in 8-pending, persisted to SQLite. Sidebar `SubsonicSyncIndicator` + `useSubsonicTrackSync` hook. Owned by the new typed slot in `AppState`.
- **Album-art hash-dedup** (`6b776bf`): `AlbumArtCache` now derives a SHA-256 content index and aliases keys pointing at byte-identical images via hardlink or copy. `provider_image_bytes_bulk` drops to a single HTTP fetch for the whole library's cover set.
- **Cover routing fixes**: `TrackTable` looks up the parent album's `imageRef` before falling back to the track's own, so per-track covers now resolve even when Subsonic returns `null` for individual songs.
- **Provider-direct reads**: `provider_list_albums/artists/tracks` + `provider_album_detail/artist_detail`, exposed in `lib/tauri.ts` as `providerList*` / `providerAlbumDetail` / `providerArtistDetail`. Snapshot list-route helper (`snapshot_list_route`) picks the Subsonic cache branch over the upstream HTTP branch per-call.
- **Favorites upstream** (`b8f25b3`): `commands::set_*_favorite` now calls `provider.set_favorite` in addition to writing the SQLite cache. New tests cover the Subsonic + Jellyfin wire shapes.
- **Lyrics.source normalization** (`84e1111`): `commands::lookup_lyrics` stamps `provider_id` onto the returned `Lyrics` if the provider forgot, so the UI provenance chip never reads "unknown".
- **Jellyfin sync parity** (`69616d0`): `kick_jellyfin_background_sync` mirrors the Subsonic guard + entry-point trio, firing from `jellyfin_login` / `provider_set_active` / `try_restore_provider`.
- **Scrobble watcher forwarding** (`7cb602b`): now also calls `provider.report_playback` (`Started` on track change, throttled `Progress` every 30 s), so Subsonic's `/rest/scrobble` and Jellyfin's `/Sessions/Playing` see Sinfonic playback even when Last.fm isn't connected.

**Total tests today: 274 Rust (`cargo test --workspace`) + 117 Vitest (`pnpm test`)** · clippy clean · pnpm build clean

---

**Estado actual:**
- `feature/cleanup-phase2` en curso: provider trait shrink (`-1,231` LOC: 17 dead `MusicProvider` methods + 7 capability flags + 2 Jellyfin `delete` helpers), domain dead types (`AppSettings`, `Route`, `QueueReplacement`, `SearchKind`, `LyricsError::NotFound`), secrets dead variants (`LibreFmSession`, `ListenBrainzToken`, `SecretError::NotFound`), lint sweep, real README + this history refresh.
- clippy clean · pnpm build clean
