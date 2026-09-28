import type { Edge, Node } from "@xyflow/react";
import type { MindDocument } from "@/shared/types/domain/mind";

/** Tree structure and sibling order are domain facts; screen positions are derived here. */
export function projectMindMap(
  document: MindDocument,
  collapsed: ReadonlySet<string>,
): { nodes: Node[]; edges: Edge[] } {
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
            (children.get(id) ?? []).reduce((total, child) => total + (heights.get(child) ?? 1), 0),
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
    nodes.push({
      id,
      type: "mind",
      position: { x: depth * 280, y: (start + (heights.get(id) ?? 1) / 2) * 100 },
      data: { label: node.content, collapsed: collapsed.has(id), hasChildren: children.has(id) },
    });
    if (node.parentId !== null)
      edges.push({ id: `parent:${id}`, source: node.parentId, target: id, type: "smoothstep" });
  }
  return { nodes, edges };
}
