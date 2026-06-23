# Sinfonic — Plan de Implementación UI estilo Rufin

**Fecha:** Junio 2026
**Proyecto:** Sinfonic (Tauri v2 + React 19 + TypeScript)
**Rama:** `feature/ui-redesign`
**Estado:** ✅ Implementado

---

## Resumen del estado actual

- ✅ `HomeView` con secciones horizontales scrolleables, Genre chips, AlbumCard, ArtistCard
- ✅ `Sidebar` con secciones colapsables (Navigation expandida, Playlists colapsada) + SourceSelector abajo
- ✅ `QueuePanel` como panel deslizable desde PlayerBar (no ruta)
- ✅ `TitleBar` con flechas de navegación (back/forward) + título dinámico
- ✅ `LoginDialog` integrado como modal, usado por SourceSelector
- ✅ `useServerForms` hook extraído de ServerManager

---

## Features implementadas

### Feature 1: Flechas de navegación en TitleBar

**Archivos:** `src/components/layout/TitleBar.tsx`

- `ArrowLeft01Icon` + `ArrowRight01Icon` de hugeicons
- Hook `useBackForward` con `popstate` listener y `window.history.state.idx`
- Título dinámico desde `ROUTE_TITLES` map
- Layout: `[←][→] [título] [Search] [Settings]`

### Feature 2: Sidebar con secciones colapsables + SourceSelector abajo

**Archivos:** `src/components/layout/Sidebar.tsx`, `src/components/layout/SourceSelector.tsx`

- CollapsibleSection con `ArrowDown01Icon`/`ArrowRight01Icon`
- Navigation (expandida por defecto) + Playlists (colapsada por defecto)
- SourceSelector fijo en la parte inferior del sidebar

### Feature 3: QueuePanel deslizable desde PlayerBar

**Archivos:** `src/components/layout/PlayerBar.tsx`, `src/components/layout/QueuePanel.tsx`, `src/components/layout/Layout.tsx`

- Botón "Queue" en PlayerBar (entre EQ y Volume)
- `queueOpen` state en Layout, pasa `queueOpen` + `onToggleQueue` a PlayerBar
- QueuePanel: `absolute inset-y-0 right-0 w-80`, se superpone al contenido
- Contenido principal con `mr-80` cuando queueOpen

### Feature 4: HomeView con secciones horizontales scrolleables

**Archivos:** `src/components/views/HomeView.tsx`, `src/components/ui/HorizontalSection.tsx`, `src/components/ui/AlbumCard.tsx`, `src/components/ui/ArtistCard.tsx`, `src/components/ui/GenreChip.tsx`

- HorizontalSection: `overflow-x-auto`, scroll snap, flechas ← → en header
- AlbumCard: cover + título + artista, link a `/library/album/:id`
- ArtistCard: avatar circular con inicial + nombre + albumCount
- GenreChip: pills placeholder con PLACEHOLDER_GENRES hardcodeados
- HomeView: Welcome header + Recently Added + Artists + Genres

### Feature 5: LoginDialog integrado

**Archivos:** `src/components/dialogs/LoginDialog.tsx`, `src/hooks/useServerForms.ts`

- Modal con `<dialog>` nativo + backdrop blur
- 3 formularios: Jellyfin, Subsonic, Local
- `useServerForms` extraído con toda la lógica de state + handlers
- SourceSelector abre LoginDialog cuando no hay servidor activo

---

## Commits en `feature/ui-redesign`

| Fecha | Commit | Descripción |
|-------|--------|-------------|
| Jun 2026 | `82 files` | Phase 10 — UI redesign: tokens, TitleBar, Settings window, themes |
| Jun 2026 | `ce63a9b` | feat(ui): navigation, source selector, queue panel, collapsible sidebar, horizontal sections |

---

## Archivos creados

- `src/components/dialogs/LoginDialog.tsx`
- `src/components/layout/SourceSelector.tsx`
- `src/components/layout/QueuePanel.tsx`
- `src/components/ui/HorizontalSection.tsx`
- `src/components/ui/AlbumCard.tsx`
- `src/components/ui/ArtistCard.tsx`
- `src/components/ui/GenreChip.tsx`
- `src/hooks/useServerForms.ts`

## Archivos modificados

- `src/components/layout/TitleBar.tsx`
- `src/components/layout/Sidebar.tsx`
- `src/components/layout/Layout.tsx`
- `src/components/layout/PlayerBar.tsx`
- `src/components/views/HomeView.tsx`

---

## Verificación

```bash
cd Sinfonic && pnpm tsc --noEmit    # ✅ Pasa clean
cd Sinfonic && pnpm build           # ✅ 145 modules, index.html + settings.html
cd Sinfonic/src-tauri && cargo check # ✅ Pasa green
```

## Siguientes pasos (backlog)

- [ ] Conectar Genre chips a datos reales del backend
- [ ] "Continue Listening" con tracks recientemente reproducidos
- [ ] Drag-to-reorder en QueuePanel
- [ ] Empty states con CTA para conectar servidor
