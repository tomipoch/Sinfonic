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

**Total tests: 148** (129 base + 6 album_art unit + 5 lastfm unit + 5 lastfm integration + 3 lastfm.rs unit)

---

## 🔜 Siguiente

### Fase 8 — Local-files provider
- Scan recursivo de `~/Music` con `lofty` (MP3/FLAC/OGG/Opus/MP4)
- Persistencia en la misma tabla SQLite que Fase 2 (server-scope = `local`)
- Stream `file://` URI al `AudioPlayer` sin cambios en playback

---

**Estado actual:**
- `develop` HEAD: pendiente del merge de Fase 7
- 148 tests · clippy clean · pnpm build clean