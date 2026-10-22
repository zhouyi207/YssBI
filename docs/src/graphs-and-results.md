# Graphs and Results

Graphs describe how data moves between analysis nodes. Edit an Event graph to build an analysis, run it explicitly, and inspect the resulting tables, plots, or statistical reports.

## Build a graph

1. Open a project and import the data you need. Work on a project copy when experimenting.
2. Create an Event graph from the File menu, or open an existing graph from the project sidebar.
3. Add nodes from the Nodes directory, the canvas menu, or **Add node** on an empty graph. With the canvas focused, Ctrl+Space (Cmd+Space on macOS) opens node search; Escape dismisses it and restores canvas focus. Search by a node's name or technical term. Unavailable nodes remain disabled in the desktop catalog.
4. Select a node to configure it in Details. Apply a field explicitly, or leave it pending for Save or Run to validate and apply. Leaving a parameter field does not implicitly apply it; switching nodes or graph tabs preserves its draft. Some supported scalar inputs can also be entered directly on ports.
5. Drag between compatible ports to connect nodes. Check Problems for missing resources, invalid parameters, incompatible types, or unresolved ports.

Input ports are on the left and output ports on the right. Pan with the right or middle mouse button, zoom around the pointer, press Home to fit the graph, and press F to locate the selected nodes. Use Undo/Redo for graph edits.

Details places a diagnostic summary and editable parameters before the full problem list. Expand Documentation for node help and Technical details for identities or a complete diagnostic report. Narrow windows temporarily collapse sidebars to preserve editing space; reopen them from the status bar. Widening the window restores automatically collapsed sidebars without replacing your saved layout.

Function graphs define reusable computations and signatures. They are not standalone Event graphs: use them through the relevant calling nodes rather than expecting the full-graph run control to execute a Function graph directly.

## Read node help

Select a node to read its help in Details, or preview a type from the Nodes directory. The [Node Reference](nodes.md) provides the same file-backed help in English and Simplified Chinese, including inputs, parameters, formulas, outputs, and interpretation.

The reference covers the maintained help pages, not every generated or resource-specific catalog entry. Use the desktop catalog to check whether a node is available in your session.

## Understand dependencies

Connections carry data dependencies, not a sequence of imperative steps. A downstream node needs the appropriate upstream values. Parameters, connected types, database schema, graph constants, and referenced functions contribute to the graph's resolved meaning.

Some output columns are known only after execution. A successful run can update column choices and derived ports without changing or saving the graph document. If a previously referenced column or port disappears, the graph retains the reference and reports the problem so you can repair it.

A node appearing in the catalog does not guarantee that your configured instance is ready to run. Its required ports, resources, and function dependencies must also be valid.

## Save and execute separately

Ordinary graph edits update the project's current in-memory graph. **Save writes the graph body; Execute computes results. Neither is a substitute for the other.** Saving preserves undo and redo. Undo and redo restore graph edits, not discarded execution results.

Use the graph toolbar for full-graph execution. Local execution supports a single node using current inputs, or computing its dependencies first. The latter is useful when upstream values are missing. Local readiness is checked for the requested dependency scope; an unrelated unfinished branch need not block it.

Save and Run first validate pending parameter and port inputs, then commit them together as one undoable edit. Invalid input blocks the operation and remains available for correction. If the underlying field changed, review the conflict and use its restore control to adopt the current value before editing again. Pending input counts as unsaved work for closing protection.

A blocked or busy run control explains why execution is unavailable. Use Cancel to request cancellation, then inspect the reported terminal state rather than assuming that clicking it immediately stopped every operation.

Editing relevant inputs discards affected results. Once a run is admitted, outputs being recomputed and their dependent results are removed; failure or cancellation does not restore previous values. Unaffected valid inputs remain available for local execution. Inspecting an existing output does not run its node automatically.

## Choose the right panel

All four panels cover the whole project, not only the graph you are editing, and switching tabs does not change them.

| Panel    | Use it for                                                                                                                                                  |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problems | Diagnostics of every graph, grouped by graph, including graphs that are not open: severity, blocking status, and links to the affected location.           |
| Output   | Runs of every graph since the project opened, newest first, with the execution phase and stable error code of each failure.                                 |
| Results  | Output ports of every event graph and whether each is computed, computing or not computed. By default only computed ports are listed; show all to see the rest. |
| Logs     | Application diagnostic records, filtered by domain, level, or text. These are not the graph's authoritative Problems or execution state.                    |

Fix the graph to resolve Problems; there is no manual clear operation for canonical diagnostics. Clearing Output acknowledges the finished runs it lists: their notices also disappear from the graphs, but cached results stay, the runs are not undone and nothing is saved. Clearing Logs clears the display, not the underlying persistent log history.

Results exist only while the project is open. After reopening the project every port is not computed until you run the graph again.

## Inspect a result

Open a result from Results or an output port's inspection action. Selecting a port that is not computed in Results computes it, reusing any valid upstream results, and opens it when the run finishes; the graph does not need to be open. Connected input ports can inspect their upstream output. Tables load bounded pages; structured results can expose nested tables, and supported statistical reports offer numerical and report views. These views use backend values rather than recomputing statistics in the interface.

An open result tab retains the specific execution result you selected. Rerunning the graph does not silently turn that tab into the new result. This explicitly held snapshot is not part of the current Results list or port inspection; open the new output to inspect the new computation.

You can open a result in an independent native window. Each window retains its own result reference. Closing a source tab does not by itself invalidate an already retained window, but a project or execution-session change can release the old results. Result tabs are not durable files and are not saved or restored with the workbench layout.

Reading or paging an existing result does not execute the graph. An explicit report action that appends analysis is different: it edits the relevant Summary configuration and runs the requested output. That edit remains subject to the normal graph save boundary.

## Further reading

- [Windows and Projects](windows-and-projects.md) covers saving, closing, and project copies.
- [Architecture](development/architecture.md) explains graph state and result ownership.
- The [Graph application contract](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-application/src/graph/README.md) owns execution and result semantics.
- [Open graph and result acceptance](https://github.com/zhouyi207/YssBI/blob/main/TODO.md#graph-and-results-acceptance) tracks remaining lifecycle and interaction checks. Implemented controls are not evidence that all of those scenarios have passed.
