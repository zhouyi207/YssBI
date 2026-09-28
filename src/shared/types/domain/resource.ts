export const FILE_RESOURCE_KINDS = [
  "event_graph",
  "function_graph",
  "chart",
  "mind",
  "doc",
] as const;
export type FileResourceKind = (typeof FILE_RESOURCE_KINDS)[number];
export const RESOURCE_KINDS = [...FILE_RESOURCE_KINDS, "database"] as const;
export type ResourceKind = (typeof RESOURCE_KINDS)[number];
export interface ResourceRef {
  kind: ResourceKind;
  id: string;
}

export type ResourceUri = `yssbi://${ResourceKind}/${string}`;
/** File kinds are independent namespaces; IDs are opaque and never classified by filename. */
export function toResourceUri(kind: ResourceKind, id: string): ResourceUri {
  return `yssbi://${kind}/${encodeURIComponent(id)}`;
}

/** File kinds whose contents use the shared node-graph editor. */
export type NodeFileKind = Extract<FileResourceKind, "event_graph" | "function_graph">;
