# Log Text Selection and Worksheet Chart Selection Design

## Summary

Worksheet chart labels must behave as non-selectable visualization chrome, while
all visible log text must support ordinary mouse selection and system copy
shortcuts for debugging.

The change is presentation-only. It does not add clipboard commands, copy
buttons, toast messages, or new application state.

## Goals

- Prevent worksheet chart axis labels, tick labels, legends, and other rendered
  chart text from being selected by mouse drag.
- Keep worksheet error messages and empty-state text selectable.
- Make every visible log field selectable in the log list: timestamp, level,
  type, source, and message.
- Make every visible log field selectable in the Detail panel: timestamp,
  level, type, source, and message.
- Preserve ordinary single-click log selection and Detail routing.
- Prevent a text-selection drag inside a log row from triggering row selection.
- Preserve log virtualization, scrolling, filtering, and keyboard behavior.

## Non-goals

- Adding a Clipboard API service or invoking Tauri clipboard commands.
- Adding copy buttons, context-menu actions, keyboard shortcuts, or copy toasts.
- Changing log persistence, payloads, filtering, or selection state.
- Changing chart zoom, pan, tooltip, pointer, or rendering behavior.
- Applying global text-selection rules to unrelated screens.

## Selected approach

Use explicit component-boundary selection styles.

Global CSS selectors are rejected because they would couple behavior to DOM
structure and could unintentionally affect unrelated controls. A reusable
selectable-text abstraction is also rejected because only the log presentation
boundary needs this behavior and a new abstraction would not reduce meaningful
duplication.

## Worksheet chart behavior

`WorksheetChartPreview` will apply non-selection behavior only to the mounted
chart region. Histogram, line, and scatter chart labels inherit this boundary,
including axis titles, tick values, legends, and other chart-rendered text.

The following remain outside the non-selection boundary:

- worksheet error messages;
- worksheet empty states;
- loading overlays.

The selection style must not disable pointer events. Existing chart hover,
tooltip, zoom, pan, and other pointer behavior must remain unchanged.

## Log list behavior

Every visible textual field in `LogItemRow` is selectable:

- timestamp;
- level;
- log type;
- source;
- message.

The row remains an interactive control for ordinary click selection. A pointer
gesture that creates or extends a text selection must not invoke the row's log
selection callback when the gesture ends. A normal click without text selection
continues to select the log and route it to Detail.

The implementation should use the browser selection state or equivalent
pointer-gesture evidence rather than timing thresholds. It must not disable
virtual-list scrolling.

## Log Detail behavior

`Detail` currently establishes a non-selectable sidebar boundary. `LogDetailPanel`
will explicitly override that inherited rule for its content.

All displayed log values are selectable:

- timestamp;
- level badge text;
- type badge text;
- source;
- multiline message text.

Selection is ordinary browser/WebView selection. Users copy with the platform's
standard copy command. Message whitespace and line breaks remain unchanged.

The override is local to `LogDetailPanel`; other Detail panels retain their
existing selection behavior.

## Accessibility and interaction

- Existing button semantics and focus behavior for log rows remain intact.
- Text selection must not require a custom keyboard interaction.
- Selection styling uses the existing platform selection colors.
- No `pointer-events: none` is introduced on log text or chart containers.
- Dragging selected text must not mutate application state.

## Testing

Focused frontend tests will verify:

1. The worksheet chart mount boundary is non-selectable.
2. Worksheet error and empty-state text are not placed inside that boundary.
3. Log list timestamp, level, type, source, and message inherit selectable text
   behavior.
4. An ordinary log-row click still invokes the selection callback exactly once.
5. A drag gesture that produces a non-collapsed text selection does not invoke
   the row selection callback.
6. Log Detail timestamp, level, type, source, and message override the parent
   `select-none` boundary.
7. Multiline log messages retain their existing `pre`/whitespace rendering.

## Validation

Run from the repository root:

```sh
pnpm test src/views/LogView/LogItemRow.test.tsx
pnpm test src/views/EditorView/Layout/Detail/panels/LogDetailPanel.test.tsx
pnpm test src/views/EditorView/Worksheet/WorksheetChartPreview.selection.test.tsx
pnpm typecheck
pnpm test
pnpm verify
git diff --check
```

If existing test filenames are extended instead of creating the listed focused
files, the implementation plan must name the exact final paths and commands.

## Completion criteria

- Worksheet chart-rendered labels cannot be selected by mouse drag.
- Worksheet error and empty-state text remain selectable.
- Every visible log field in the list and Detail can be selected and copied with
  standard platform behavior.
- Dragging to select log-row text does not select or switch the log item.
- Ordinary single-click log selection still works.
- No unrelated screen receives a selection-style change.
- Focused tests, typecheck, the frontend suite, canonical verification, and
  `git diff --check` pass.
