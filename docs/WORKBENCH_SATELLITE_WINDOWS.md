# Workbench Satellite Windows

YssBI uses two window shells:

## Main Workbench (`EditorWindow`)

- Full VS Code-style layout: Activity Bar, Sidebar, Editor Grid, Panel, Detail, Status Bar.
- Layout persisted via `workbenchLayoutMemento` (`localStorage` key `yssbi-workbench-layout`).
- Multiple editor windows share project state; secondary window geometry uses `usePersistedSecondaryWindow`.

## Presentation Windows (`PresentationWindowShell`)

Used for focused tools that are **not** workbench Parts:

| Window | Purpose |
|--------|---------|
| Plot | Chart preview |
| Database Editor | Data tables |
| Source Inspector | Resource metadata |
| Log (detached) | Log panel torn off via HTML5 DnD from embedded panel |

These windows **do not** read/write the workbench layout tree. They reuse theme/settings sync (`CLIENT_SETTINGS_UPDATED_EVENT`).

## Detaching Panel Views

- **Logs**: drag handle in `LogPanelContent` → standalone `LogWindow` (reference implementation).
- Future Terminal/Webview panels should follow the same pattern: embedded in `PanelPart` tab strip, optional detach to satellite shell.

## Boundaries

- Do **not** mount workbench `LayoutNodeRenderer` inside satellite windows.
- Part resize / sash APIs apply only to the main workbench window.
- Editor tabs and graph documents remain tied to the main workbench editor grid unless explicitly designed otherwise.
