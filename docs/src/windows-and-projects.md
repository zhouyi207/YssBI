# Windows and Projects

A project holds your analysis resources: databases, Event and Function graphs, charts, Markdown documents, and Mind documents. Open its directory to work with these resources in the native workbench.

## Open or create a project

Start the application with `cargo run` from the source checkout. The welcome page offers opening a directory, creating a project, and the six most recent projects. The recent-project picker searches by name or path; use the arrow keys and Enter to select a project, or Escape to cancel.

New projects ask for a name and location. You can also open a project at launch with the [positional CLI arguments](reference/cli.md). A failed recent-project lookup does not prevent opening a directory directly.

Use a separate copy when experimenting with an existing project. Project activation can perform directory initialization and database maintenance before you edit anything.

## Navigate the workbench

Use the sidebar to switch between the project resources, node catalog, and Assistant conversations. Opening a resource places its editor in the central area. Details shows properties for the active resource or selected graph node.

Panels and editor tabs can be rearranged in the dock layout. Splitting an editor group moves the current editor to the right or below and requires at least two tabs in the group; it does not create another copy of the graph. Region controls in the status bar show or hide the sidebars and bottom tools.

Reset Layout returns open resources to the central tabs, the resource directory to the left, Details to the right, and tools to the bottom. It preserves unsaved input and open result references rather than reopening your project.

Settings opens in a separate window through Tools → Settings or `Ctrl+,` (`Cmd+,` on macOS). Appearance includes the English and Simplified Chinese language choices.

## Save your work

Use Save for the active editor or the workbench's Save All control for pending work. Graph, document, Mind, and chart editors use `Ctrl+S` on Linux and Windows, or `Cmd+S` on macOS. In the main workbench, `Ctrl+Shift+S` or `Cmd+Shift+S` invokes project Save As, not Save All.

Saving has different meanings for different resources:

- **Graphs:** Ordinary canvas edits and undo/redo change the current in-memory document. Explicit Save writes the graph body. Running a graph does not save it.
- **Documents, Mind documents, and charts:** Keep track of both unsubmitted editor input and unsaved changes. Use the editor's apply controls where offered, then Save to persist the resource.
- **Database settings:** An applied mutation is already written to the project. Save establishes a checkpoint and ends that edit history; it is not the first disk write. Leaving without a checkpoint does not undo applied database settings.
- **Assistant changes:** Write-mode tools can persist changes through their own transactions. They are not all pending GUI edits. See [Assistant](ai/overview.md).

Save As creates a target copy through the project lifecycle. The desktop first asks how to handle unsaved input when necessary. This is not a substitute for making a safe copy _before_ opening a valuable project.

## Switch, close, and recover

Opening another project, creating a project, closing the project, and quitting use the workbench's save checks. Choose to save, continue without saving, or cancel as offered. Closing a project returns to the welcome page.

Dirty resource tabs cannot simply be closed without resolving their pending work. On window exit, discarding unsaved file or model-setting input does not revert database settings already committed. A save failure keeps the relevant input and prevents the pending switch or exit from proceeding.

A failed create or Save As operation may already have written its target. Read the failure message and recovery path before retrying. If the workbench offers to open the written project, use that recovery route rather than creating the same target again.

Result tabs can open the same result in an independent native window. A normal rerun does not replace that window's snapshot. Project replacement, execution-session changes, or closing the main workbench can close those result windows. See [Graphs and Results](graphs-and-results.md).

## Implementation and acceptance

These workflows are implemented, but project recovery, layout restoration, close protection, and platform-specific window behavior still have open manual acceptance work. The [desktop contract](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-gpui/README.md) documents the native behavior; the [Project contract](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-project/README.md) defines persistence and resource ownership.
