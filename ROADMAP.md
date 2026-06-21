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

**Total tests: 108**

---

## 🔜 Siguiente

Las dos ramas más naturales son:

### Fase 5 — Provider Subsonic (paridad con Jellyfin)
- Implementar `MusicProvider` para Subsonic API (Navidrome / Funkwhale / Airsonic)
- Auth: username + token (salt + md5)
- Endpoints: `ping`, `getLicense`, `getMusicFolders`, `getArtists`, `getAlbum`, `getAlbumList2`, `search3`, `stream`, `scrobble`
- Selector de provider en `SettingsView`
- Tests con `wiremock` + auth salt-md5
- **Por qué primero:** completa la historia multi-source y desbloquea servidores auto-hospedados sin Jellyfin.

### Fase 5 (alternativa) — Local-files provider
- Scan recursivo de un directorio configurable (default `~/Music`)
- Parse de metadatos con `lofty` (MP3/FLAC/OGG/Opus/MP4)
- Persistencia en la misma tabla SQLite que Fase 2 (server-scope = `local`)
- **Por qué:** más simple, no requiere servidor externo, ideal para testing.

### Fase 6 — UI real (LibraryView / AlbumDetailView / MiniPlayer)
- Reemplazar `EmptyView` con `LibraryView` browse de albums/artists/tracks
- `AlbumDetailView` con lista de tracks y botón play-album
- MiniPlayer con seek bar, volumen, mute, EQ panel
- Búsqueda global en el sidebar

### Fase 7 — Album art + scrobble polish
- Caché local de carátulas (LRU + filesystem)
- `last_fm_scrobble` opcional

---

**Estado actual:**
- `develop` HEAD: `c682b2c`
- 108 tests · clippy clean · pnpm build clean