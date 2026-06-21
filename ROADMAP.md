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

**Total tests: 129**

---

## 🔜 Siguiente

### Fase 7 — Album art + scrobble polish + EQ UI
- **Album art cache**: filesystem LRU bajo `app_data_dir/album_art/`, clave SHA-256(provider_image_id + image_tag), comando `provider_image_bytes(album_id)` read-through con eviction por tamaño
- **AlbumCover real**: `<img>` con `URL.createObjectURL` + fallback al gradiente cuando no hay bytes
- **Pre-warm**: hook dispara fetch en background para los primeros N álbumes al cargar la librería
- **Last.fm scrobble**: `sinfonic-lastfm` crate (handshake `auth.getMobileSession` + `track.updateNowPlaying` + `track.scrobble` con firma md5(api_secret + timestamp))
- **Credenciales en keyring**: `SecretKey::LastFmApiSecret` + `LastFmSession` (variantes ya existían sin uso)
- **ScrobbleWatcher**: task en background escucha `PlayerEvent::StateChanged` (cada 250 ms) + `TrackEnded` → dispara now-playing al cambiar track y scrobble al pasar 50 % (o al terminar)
- **Settings UI**: sección Last.fm (api_key + api_secret + username + password + Connect/Test/Disconnect)
- **EQ panel**: comando `get_eq_bands` (lee del `AudioPlayer`), `useEqStore`, `EqPanel` con 10 sliders (60 Hz … 16 kHz, ±12 dB) + botón reset, popover colapsable dentro de `PlayerBar`

### Fase 8 — Local-files provider
- Scan recursivo de `~/Music` con `lofty` (MP3/FLAC/OGG/Opus/MP4)
- Persistencia en la misma tabla SQLite que Fase 2 (server-scope = `local`)
- Stream `file://` URI al `AudioPlayer` sin cambios en playback

---

**Estado actual:**
- `develop` HEAD: `42ab555` (Phase 6 merged)
- 129 tests · clippy clean · pnpm build clean