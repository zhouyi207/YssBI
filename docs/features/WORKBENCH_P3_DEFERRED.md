# Workbench P3 — Implemented Items

## Native vs custom title bar (`window.titleBarStyle`)

**Status: implemented**

- Setting: `appearance.titleBarStyle` → `"custom"` | `"native"` (Settings → Appearance)
- Policy: `windowDecorationPolicy.ts` + `useWindowDecorationEffect()` (global via `SettingsEffectsProvider`)
- New windows: `createPersistedWindow` reads the current setting at creation time
- Runtime toggle: `getCurrentWindow().setDecorations()` when the setting changes (all webviews)
- UI: `WindowChrome` / `WindowMenuBar` hide custom chrome when native; toolbar buttons remain

## Status bar command registry

**Status: implemented (v1)**

- Core: `features/core/statusBar/` — `registerStatusBarItem()`, built-in descriptors, `buildStatusBarItems()`
- Application: `useStatusBarItems()` resolves store context + built-in + registered items
- View: `BottomBar` maps view models → `StatusBarItem`

Extensions can call `registerStatusBarItem({ id, alignment, priority, render, onClick, ... })` at module load.

## Zen mode hint overlay

**Status: implemented**

- `ZenModeHintOverlay` in `EditorWindow` — bottom-center pill, auto-dismiss, `Esc` key hint
- i18n: `workbench.exitZenModeHint`
