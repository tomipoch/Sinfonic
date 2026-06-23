# Sinfonic — Plan de Implementación UI estilo Rufin

**Fecha:** Junio 2026
**Proyecto:** Sinfonic (Tauri v2 + React 19 + TypeScript)

---

## Resumen del estado actual

- `HomeView` es un placeholder vacío
- `Sidebar` solo tiene NavLinks sin estructura collapsible ni SourceSelector
- `QueueView` es una ruta completa, no un panel deslizable
- `TitleBar` no tiene flechas de navegación ni título dinámico
- No hay Genre chips ni sección horizontal scrolleable
- No hay LoginDialog integrado — la conexión vive solo en Settings

---

## Feature 1: Flechas de navegación en TitleBar

**Archivos:** `src/components/layout/TitleBar.tsx`

**Qué hacer:**
- Añadir `ArrowLeft01Icon` y `ArrowRight01Icon` de hugeicons
- Crear un hook `useBackForward` que use `window.history` para saber si se puede hacer back/forward
- Mostrar título dinámico de la vista actual (leyendo `useLocation` pathname → formatear nombre)
- Layout: `[←][→] [título] [Search] [Settings]` — el `flex-1 data-tauri-drag-region` queda en el centro

**Dependencias:** `useLocation` de `react-router-dom`

---

## Feature 2: Sidebar con secciones colapsables + SourceSelector abajo

**Archivos:** `src/components/layout/Sidebar.tsx`, `src/components/layout/SourceSelector.tsx` (nuevo), `src/components/ui/collapsible.tsx` (nuevo o usar headless UI)

**Estructura del sidebar:**
```
┌─────────────────────────┐
│  Navigation              │  ← collapsible
│   Home                   │
│   Library                │
│   Playlists              │
│   Favorites              │
│   Smart Playlists        │
│                         │
│  ─────────────────────   │
│  Playlists              │  ← collapsible
│   ♫ Mi Playlist 1       │
│   ♫ Favoritas           │
│                         │
│  ─────────────────────   │
│  [🟢 Jellyfin]  ▼       │  ← SourceSelector (fijo abajo)
└─────────────────────────┘
```

**SourceSelector (nuevo componente):**
- Lee `useServerStore().servers` + `activeServerId`
- Muestra un dropdown con:
  - Lista de servidores guardados + marca de activo
  - Opción "Add new server..." → abre `LoginDialog`
- Si `activeServerId === null` → muestra "Connect server" → abre `LoginDialog` directamente

**CollapsibleSections:**
- Cada sección tiene: header clickeable con chevron + children
- Mantener estado `expanded` por sección en el componente padre

---

## Feature 3: Queue Panel deslizable desde PlayerBar

**Archivos:** `src/components/layout/PlayerBar.tsx`, `src/components/layout/QueuePanel.tsx` (nuevo), `src/components/layout/Layout.tsx`

**PlayerBar cambios:**
- Añadir `ListIcon` (queue icon) de hugeicons al lado derecho, entre EQ y Volume
- Botón con estado activo cuando el panel está abierto

**QueuePanel (nuevo):**
- Panel deslizable desde la derecha (transform translate, no overlay full)
- Anchura ~320px, se superpone al contenido con `position absolute right-0 top-0 bottom-0`
- Mueve contenido principal `Layout.tsx` para hacer sitio (`mr-[320px]`) cuando está abierto
- Mismo contenido que `QueueView` actual: lista de canciones con drag-to-reorder, header con shuffle/clear

**Layout cambios:**
- El `main` flex-1 ya contiene `<Outlet />` + `PlayerBar`
- Cuando `queueOpen === true`, el div flex que contiene sidebar + main se convierte en `relative overflow-hidden`
- `QueuePanel` se renderiza como `absolute right-0 top-0 bottom-0 z-40`

---

## Feature 4: HomeView con secciones horizontales scrolleables

**Archivos:** `src/components/views/HomeView.tsx`, `src/components/ui/HorizontalSection.tsx` (nuevo), `src/components/ui/AlbumCard.tsx`, `src/components/ui/ArtistCard.tsx`

**Estructura:**
```tsx
// HomeView
<HomeView>
  <WelcomeHeader />  // "Welcome to Sinfonic" o nombre del servidor
  <HorizontalSection title="Continue Listening">
    <TrackRowCard />  // tracks recientemente reproducidos
  </HorizontalSection>
  <HorizontalSection title="Recently Added">
    <AlbumCard /> × scroll
  </HorizontalSection>
  <HorizontalSection title="Artists">
    <ArtistCard /> × scroll
  </HorizontalSection>
  <HorizontalSection title="Genres">
    <GenreChip />  // pills clickeables (no horizontal scroll — wrap)
  </HorizontalSection>
</HomeView>
```

**HorizontalSection (nuevo):**
- `overflow-x-auto`, hide scrollbar con CSS trick
- Flechas `← →` en los extremos que aparecen en hover
- `snap-x snap-mandatory` para scroll suave

**Genre chips:** Por ahora no tienen datos — el botón es decorativo/hTML until backend lands. Mostrar placeholder pills en gris con "Rock", "Pop", "Jazz", etc. hardcodeados.

---

## Feature 5: LoginDialog integrado (modal)

**Archivos:** `src/components/dialogs/LoginDialog.tsx` (nuevo), `src/components/layout/SourceSelector.tsx`

**LoginDialog:**
- Modal con `radix-ui/react-dialog` (o `<dialog>` nativo)
- Mismo contenido que `ServerManager` (3 ChoiceCards para tipo de servidor + formularios Jellyfin/Subsonic/Local)
- Extrae la lógica de `ServerManager` a hooks (`useServerForm`) para reutilizar entre Settings y este dialog
- Se abre desde:
  1. SourceSelector cuando no hay servidor activo
  2. CTA "Connect a server" en empty states (AlbumsTab, FavoritesView, etc.)

**Refactor de ServerManager:**
- Extraer forms + state management a hooks (`useJellyfinForm`, `useSubsonicForm`, `useLocalForm`)
- `ServerManager` se convierte en wrapper que usa esos hooks + el diálogo
- El `LoginDialog` usa los mismos hooks

---

## Orden de implementación sugerida

| Orden | Feature | Razón |
|-------|---------|-------|
| 1 | **LoginDialog** | Dependencias mínimas, otros features lo usan |
| 2 | **SourceSelector** en Sidebar | LoginDialog necesita existir primero |
| 3 | **Flechas TitleBar** | Independiente, se puede hacer en paralelo |
| 4 | **Collapsible sections** Sidebar | Depende de SourceSelector (layout) |
| 5 | **QueuePanel** | Depende de cambios en Layout + PlayerBar |
| 6 | **HomeView + HorizontalSection** | Dependencias más complejas |

---

## Archivos a modificar/crear

**Nuevos:**
- `src/components/dialogs/LoginDialog.tsx`
- `src/components/layout/SourceSelector.tsx`
- `src/components/ui/HorizontalSection.tsx`
- `src/components/layout/QueuePanel.tsx`
- `src/hooks/useServerForms.ts` (extraído de ServerManager)

**Modificados:**
- `src/components/layout/TitleBar.tsx`
- `src/components/layout/Sidebar.tsx`
- `src/components/layout/Layout.tsx`
- `src/components/layout/PlayerBar.tsx`
- `src/components/views/HomeView.tsx`
- `src/components/settings/ServerManager.tsx` (extraer a hooks)

---

## Referencia: Diseño Rufin (GTK4/libadwaita)

Ver `ui_documentation.md` en el directorio raíz para la especificación completa del diseño de Rufin que inspiró este plan.
