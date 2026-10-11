# Graphs and Results

Graphs describe how data moves between analysis nodes. Edit an Event graph to build an analysis, run it explicitly, and inspect the resulting tables, plots, or statistical reports.

## Build a graph

1. Open a project and import the data you need. Work on a project copy when experimenting.
2. Create an Event graph from the File menu, or open an existing graph from the project sidebar.
3. Add nodes from the Nodes directory or the canvas menu. Search by a node's name or technical term. Unavailable nodes remain disabled in the desktop catalog.
4. Select a node to configure it in Details. Apply fields with their explicit controls; leaving a parameter field does not implicitly apply it. Some supported scalar inputs can also be entered directly on ports.
5. Drag between compatible ports to connect nodes. Check Problems for missing resources, invalid parameters, incompatible types, or unresolved ports.

Input ports are on the left and output ports on the right. Pan with the right or middle mouse button, zoom around the pointer, press Home to fit the graph, and press F to locate the selected nodes. Use Undo/Redo for graph edits.

Function graphs define reusable computations and signatures. They are not standalone Event graphs: use them through the relevant calling nodes rather than expecting the full-graph run control to execute a Function graph directly.

## Read node help

Select a node to read its help in Details, or preview a type from the Nodes directory. The [Node Reference](nodes.md) provides the same file-backed help in English and Simplified Chinese, including inputs, parameters, formulas, outputs, and interpretation.

The reference covers the maintained help pages, not every generated or resource-specific catalog entry. Use the desktop catalog to check whether a node is available in your session.

## Understand dependencies

Connections carry data dependencies, not a sequence of imperative steps. A downstream node needs the appropriate upstream values. Parameters, connected types, database schema, graph constants, and referenced functions contribute to the graph's resolved meaning.

Some output columns are known only after execution. A successful run can update column choices and derived ports without changing or saving the graph document. If a previously referenced column or port disappears, the graph retains the reference and reports the problem so you can repair it.

A node appearing in the catalog does not guarantee that your configured instance is ready to run. Its required ports, resources, and function dependencies must also be valid.

## Save and execute separately

Ordinary graph edits update the project's current in-memory graph. **Save writes the graph body; Execute computes results. Neither is a substitute for the other.** Undo and redo restore graph edits, not discarded execution results.

Use the graph toolbar for full-graph execution. Local execution supports a single node using current inputs, or computing its dependencies first. The latter is useful when upstream values are missing. Local readiness is checked for the requested dependency scope; an unrelated unfinished branch need not block it.

The run controls apply pending port input before preparing execution. A blocked or busy run control explains why execution is unavailable. Use Cancel to request cancellation, then inspect the reported terminal state rather than assuming that clicking it immediately stopped every operation.

Editing relevant inputs discards affected results. Once a run is admitted, outputs being recomputed and their dependent results are removed; failure or cancellation does not restore previous values. Unaffected valid inputs remain available for local execution. Inspecting an existing output does not run its node automatically.

## Choose the right panel

| Panel    | Use it for                                                                                                                                 |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Problems | Current graph diagnostics, including severity, blocking status, and links to affected nodes, parameters, ports, connections, or resources. |
| Output   | The current graph's execution status and failures, including the execution phase and stable error code when available.                     |
| Results  | Current valid graph outputs. Outputs being recomputed or awaiting synchronization are not listed.                                         |
| Logs     | Application diagnostic records, filtered by domain, level, or text. These are not the graph's authoritative Problems or execution state.   |

Fix the graph to resolve Problems; there is no manual clear operation for canonical diagnostics. Clearing a completed Output notice only acknowledges its presentation: it does not delete cached results, undo the run, or save the graph. Clearing Logs clears the display, not the underlying persistent log history.

## Inspect a result

Open a result from Results or an output port's inspection action. Connected input ports can inspect their upstream output. Tables load bounded pages; structured results can expose nested tables, and supported statistical reports offer numerical and report views. These views use backend values rather than recomputing statistics in the interface.

An open result tab retains the specific execution result you selected. Rerunning the graph does not silently turn that tab into the new result. This explicitly held snapshot is not part of the current Results list or port inspection; open the new output to inspect the new computation.

You can open a result in an independent native window. Each window retains its own result reference. Closing a source tab does not by itself invalidate an already retained window, but a project or execution-session change can release the old results. Result tabs are not durable files and are not saved or restored with the workbench layout.

Reading or paging an existing result does not execute the graph. An explicit report action that appends analysis is different: it edits the relevant Summary configuration and runs the requested output. That edit remains subject to the normal graph save boundary.

## Further reading

- [Windows and Projects](windows-and-projects.md) covers saving, closing, and project copies.
- [Architecture](development/architecture.md) explains graph state and result ownership.
- The [Graph application contract](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-application/src/graph/README.md) owns execution and result semantics.
- [Open graph and result acceptance](https://github.com/zhouyi207/YssBI/blob/main/TODO.md#graph-and-results-acceptance) tracks remaining lifecycle and interaction checks. Implemented controls are not evidence that all of those scenarios have passed.
