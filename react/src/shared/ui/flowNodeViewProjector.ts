import type { Node, NodeDimensionChange } from "@xyflow/react";

interface FlowNodeViewInputs<NodeType extends Node> {
  model: { nodes: NodeType[]; nodeIds: ReadonlySet<string> };
  positions: ReadonlyMap<string, Pick<NodeType, "position" | "dragging">>;
  selectedNodeIds: ReadonlySet<string>;
  interactive: boolean;
}

/** One panel's transient node views; committed projections and gesture ownership stay separate. */
export function createFlowNodeViewProjector<NodeType extends Node>() {
  const cache = new Map<string, { index: number; projection: NodeType; view: NodeType }>();
  let previous: FlowNodeViewInputs<NodeType> | undefined;
  let nodes: NodeType[] = [];

  const project = (input: FlowNodeViewInputs<NodeType>): NodeType[] => {
    const { model, positions, selectedNodeIds, interactive } = input;
    const before = previous;
    let next = nodes;
    const sameBasis =
      before !== undefined &&
      before.model.nodes === model.nodes &&
      before.model.nodeIds === model.nodeIds &&
      before.interactive === interactive;
    const positionsChanged = before?.positions !== positions;
    const selectionChanged = interactive && before?.selectedNodeIds !== selectedNodeIds;
    if (sameBasis && !positionsChanged && !selectionChanged) {
      previous = input;
      return nodes;
    }
    // Count only changed inputs; dense selection or drag updates use one full node pass.
    const sparseVisits = sameBasis
      ? (positionsChanged ? positions.size + before.positions.size : 0) +
        (selectionChanged ? selectedNodeIds.size + before.selectedNodeIds.size : 0)
      : model.nodes.length;
    if (sameBasis) {
      const updateView = (id: string) => {
        const entry = cache.get(id);
        if (!entry) return;
        const preview = positionsChanged ? positions.get(id) : undefined;
        const position = positionsChanged
          ? (preview?.position ?? entry.projection.position)
          : entry.view.position;
        const dragging = positionsChanged ? (preview?.dragging ?? false) : entry.view.dragging;
        const selected = selectionChanged ? selectedNodeIds.has(id) : entry.view.selected;
        if (
          entry.view.position.x === position.x &&
          entry.view.position.y === position.y &&
          entry.view.dragging === dragging &&
          entry.view.selected === selected
        )
          return;
        // React Flow consumes an immutable array. Copy it once, then replace only affected slots.
        if (next === nodes) next = nodes.slice();
        entry.view = { ...entry.view, position, dragging, selected };
        next[entry.index] = entry.view;
      };
      if (sparseVisits < model.nodes.length) {
        if (positionsChanged) {
          for (const [id, preview] of positions) {
            if (preview !== before.positions.get(id)) updateView(id);
          }
          // Clearing a gesture restores its committed position without scanning unrelated nodes.
          for (const id of before.positions.keys()) {
            if (!positions.has(id)) updateView(id);
          }
        }
        if (selectionChanged) {
          // A node present in both diffs already has its complete view after the first visit.
          for (const id of selectedNodeIds) {
            if (!before.selectedNodeIds.has(id)) updateView(id);
          }
          for (const id of before.selectedNodeIds) {
            if (!selectedNodeIds.has(id)) updateView(id);
          }
        }
      } else {
        for (const node of nodes) updateView(node.id);
      }
    } else {
      let changed = model.nodes.length !== nodes.length;
      next = model.nodes.map((node, index) => {
        const preview = positions.get(node.id);
        const position = preview?.position ?? node.position;
        const dragging = preview?.dragging ?? false;
        const selected = interactive && selectedNodeIds.has(node.id);
        const selectable = interactive && node.selectable;
        const draggable = interactive && node.draggable;
        const entry = cache.get(node.id);
        let view = entry?.view;
        const measured = view?.measured;
        if (
          entry?.projection !== node ||
          !view ||
          view.position.x !== position.x ||
          view.position.y !== position.y ||
          view.dragging !== dragging ||
          view.selected !== selected ||
          view.selectable !== selectable ||
          view.draggable !== draggable
        ) {
          view = { ...node, position, measured, dragging, selected, selectable, draggable };
          if (entry) {
            entry.index = index;
            entry.projection = node;
            entry.view = view;
          } else {
            cache.set(node.id, { index, projection: node, view });
          }
        } else {
          entry.index = index;
        }
        if (view !== nodes[index]) changed = true;
        return view;
      });
      if (!changed) next = nodes;
      if (before?.model.nodeIds !== model.nodeIds) {
        for (const id of cache.keys()) {
          if (!model.nodeIds.has(id)) cache.delete(id);
        }
      }
    }
    previous = input;
    nodes = next;
    return nodes;
  };

  const updateMeasurements = (changes: readonly NodeDimensionChange[]): boolean => {
    let next = nodes;
    for (const change of changes) {
      const entry = cache.get(change.id);
      const size = change.dimensions;
      if (
        !entry ||
        !size ||
        !Number.isFinite(size.width) ||
        !Number.isFinite(size.height) ||
        size.width <= 0 ||
        size.height <= 0 ||
        (entry.view.measured?.width === size.width && entry.view.measured.height === size.height)
      )
        continue;
      // Measurements belong to the existing view entry; no parallel dimension table
      // or full projection scan is needed for a localized resize.
      if (next === nodes) next = nodes.slice();
      entry.view = { ...entry.view, measured: { width: size.width, height: size.height } };
      next[entry.index] = entry.view;
    }
    if (next === nodes) return false;
    nodes = next;
    return true;
  };

  return { project, updateMeasurements };
}
