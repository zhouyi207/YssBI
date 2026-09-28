import type { FileSnapshot, FileCommand, FileIndexEntry } from "./fileDocument";
export type MindReference =
  | { kind: "resource"; path: string }
  | { kind: "graph_node"; path: string; nodeId: string }
  | { kind: "database"; databaseId: string };
export interface MindNode {
  id: string;
  parentId: string | null;
  content: string;
  position?: { x: number; y: number };
  reference?: MindReference;
}
export interface MindDocument {
  rootId: string;
  nodes: MindNode[];
}
export type MindEdit =
  | { op: "add_node"; node: MindNode }
  | { op: "set_content"; nodeId: string; content: string }
  | { op: "set_position"; nodeId: string; position: { x: number; y: number } | null }
  | { op: "set_reference"; nodeId: string; reference: MindReference | null }
  | { op: "move_node"; nodeId: string; parentId: string; beforeId: string | null }
  | { op: "remove_node"; nodeId: string };
export type MindSnapshot = FileSnapshot<"mind", MindDocument>;
export type MindCommand = FileCommand<MindEdit>;
export type MindIndexEntry = FileIndexEntry<"mind">;
export function isMindPath(path: unknown): path is string {
  return typeof path === "string" && /^minds\/[^/\\]+\.yssbi-mind$/.test(path);
}
