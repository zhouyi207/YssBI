import type {
  MindDocument,
  MindEdit,
  MindReference,
  MindSnapshot,
} from "@/shared/types/domain/mind";
import { isMindPath } from "@/shared/types/domain/mind";
import {
  record,
  exact,
  parseFileSnapshot,
  createFileContentService,
} from "@/services/project/fileContentService";
function reference(value: unknown): value is MindReference {
  if (!record(value)) return false;
  if (value.kind === "resource")
    return exact(value, ["kind", "path"]) && typeof value.path === "string";
  if (value.kind === "graph_node")
    return (
      exact(value, ["kind", "path", "nodeId"]) &&
      typeof value.path === "string" &&
      typeof value.nodeId === "string"
    );
  return (
    value.kind === "database" &&
    exact(value, ["kind", "databaseId"]) &&
    typeof value.databaseId === "string"
  );
}
function parseMind(value: unknown): MindDocument {
  const mind = value;
  if (
    !record(mind) ||
    !exact(mind, ["rootId", "nodes"]) ||
    typeof mind.rootId !== "string" ||
    !Array.isArray(mind.nodes) ||
    mind.nodes.length < 1 ||
    mind.nodes.length > 5_000
  )
    throw new Error("Invalid mind document");
  const parents = new Map<string, string | null>();
  for (const node of mind.nodes) {
    if (
      !record(node) ||
      !exact(node, ["id", "parentId", "content"], ["reference"]) ||
      typeof node.id !== "string" ||
      !node.id ||
      parents.has(node.id) ||
      (node.parentId !== null && typeof node.parentId !== "string") ||
      typeof node.content !== "string" ||
      (node.reference !== undefined && !reference(node.reference))
    )
      throw new Error("Invalid mind node");
    parents.set(node.id, node.parentId);
  }
  if (parents.get(mind.rootId) !== null) throw new Error("Invalid mind hierarchy");
  const connected = new Set([mind.rootId]);
  for (const id of parents.keys()) {
    const chain = new Set<string>();
    let current = id;
    while (!connected.has(current)) {
      if (chain.has(current)) throw new Error("Invalid mind hierarchy");
      chain.add(current);
      const parent = parents.get(current);
      if (parent === null || parent === undefined) throw new Error("Invalid mind hierarchy");
      current = parent;
    }
    for (const ancestor of chain) connected.add(ancestor);
  }
  return mind as unknown as MindDocument;
}
export function parseMindSnapshot(value: unknown): MindSnapshot {
  return parseFileSnapshot(value, "mind", isMindPath, parseMind);
}
export const MindService = createFileContentService<MindSnapshot, MindEdit>(
  "read_project_mind",
  "edit_project_mind",
  parseMindSnapshot,
);
