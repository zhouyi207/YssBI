import type { FileResourceKind } from "./resource";
export interface FileVersion {
  sessionId: string;
  revision: number;
}
export interface FileSnapshot<K extends FileResourceKind, C> {
  projectInstanceId: string;
  path: string;
  version: FileVersion;
  kind: K;
  content: C;
  dirty: boolean;
}
export type FileCommand<E> =
  | { op: "create"; name: string }
  | { op: "edit"; path: string; version: FileVersion; edits: E[] }
  | { op: "save" | "discard" | "duplicate" | "delete"; path: string; version: FileVersion }
  | { op: "rename"; path: string; version: FileVersion; name: string };
export interface FileIndexEntry<K extends FileResourceKind> {
  path: string;
  kind: K;
  name: string;
  revision: number;
}
