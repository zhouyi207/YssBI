import type { FileSnapshot, FileCommand, FileIndexEntry } from "./fileDocument";
export type DocDocument = string;
export type DocEdit = { op: "set_markdown"; markdown: string };
export type DocSnapshot = FileSnapshot<"doc", DocDocument>;
export type DocCommand = FileCommand<DocEdit>;
export type DocIndexEntry = FileIndexEntry<"doc">;
export function isDocPath(path: unknown): path is string {
  return typeof path === "string" && /^docs\/[^/\\]+\.md$/.test(path);
}
