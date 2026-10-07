import type { Edge, Node } from "@xyflow/react";
import { shallow } from "zustand/shallow";
import type { MindDocument } from "@/shared/types/domain/mind";

/** Tree structure and sibling order are domain facts; screen positions are derived here. */
export interface MindMapProjection {
  nodes: Node[];
  nodeIds: ReadonlySet<string>;
  edges: Edge[];
}

export function createMindMapProjector() {
  let model: MindMapProjection = { nodes: [], nodeIds: new Set(), edges: [] };
  const nodesById = new Map<string, Node>();
  const edgesByChildId = new Map<string, Edge>();
  return (document: MindDocument, collapsed: ReadonlySet<string>): MindMapProjection => {
    const byId = new Map(document.nodes.map((node) => [node.id, node]));
    const children = new Map<string, string[]>();
    for (const node of document.nodes) {
      if (node.parentId !== null) {
        const group = children.get(node.parentId) ?? [];
        group.push(node.id);
        children.set(node.parentId, group);
      }
    }
    const order: Array<{ id: string; depth: number }> = [];
    const pending = [{ id: document.rootId, depth: 0 }];
    const visited = new Set<string>();
    while (pending.length) {
      const entry = pending.pop()!;
      if (visited.has(entry.id)) continue;
      visited.add(entry.id);
      order.push(entry);
      if (!collapsed.has(entry.id))
        for (const child of [...(children.get(entry.id) ?? [])].reverse())
          pending.push({ id: child, depth: entry.depth + 1 });
    }
    const heights = new Map<string, number>();
    for (const { id } of [...order].reverse())
      heights.set(
        id,
        collapsed.has(id)
          ? 1
          : Math.max(
              1,
              (children.get(id) ?? []).reduce(
                (total, child) => total + (heights.get(child) ?? 1),
                0,
              ),
            ),
      );
    const starts = new Map<string, number>([[document.rootId, 0]]);
    const nodes: Node[] = [];
    const edges: Edge[] = [];
    for (const { id, depth } of order) {
      const node = byId.get(id)!;
      const start = starts.get(id) ?? 0;
      let next = start;
      for (const child of children.get(id) ?? []) {
        starts.set(child, next);
        next += heights.get(child) ?? 1;
      }
      const old = nodesById.get(id);
      const x = depth * 280;
      const y = (start + (heights.get(id) ?? 1) / 2) * 100;
      const isCollapsed = collapsed.has(id);
      const position = old?.position.x === x && old.position.y === y ? old.position : { x, y };
      const data =
        old?.data.label === node.content && old.data.collapsed === isCollapsed
          ? old.data
          : { label: node.content, collapsed: isCollapsed };
      const view =
        old && old.position === position && old.data === data
          ? old
          : { id, type: "mind", position, data };
      nodesById.set(id, view);
      nodes.push(view);
      if (node.parentId !== null) {
        const oldEdge = edgesByChildId.get(id);
        const edge =
          oldEdge?.source === node.parentId
            ? oldEdge
            : { id: `parent:${id}`, source: node.parentId, target: id, type: "smoothstep" };
        edgesByChildId.set(id, edge);
        edges.push(edge);
      }
    }
    for (const id of nodesById.keys()) if (!visited.has(id)) nodesById.delete(id);
    for (const id of edgesByChildId.keys()) {
      if (!visited.has(id) || byId.get(id)?.parentId === null) edgesByChildId.delete(id);
    }
    const nextNodes = shallow(nodes, model.nodes) ? model.nodes : nodes;
    const nextEdges = shallow(edges, model.edges) ? model.edges : edges;
    const nodeIds = shallow(visited, model.nodeIds) ? model.nodeIds : visited;
    if (nextNodes !== model.nodes || nextEdges !== model.edges || nodeIds !== model.nodeIds) {
      model = { nodes: nextNodes, nodeIds, edges: nextEdges };
    }
    return model;
  };
}
