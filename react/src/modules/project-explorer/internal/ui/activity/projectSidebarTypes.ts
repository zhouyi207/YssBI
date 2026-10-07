import type { RevealProjectResourceRequest } from "@/features/application/sidebar";
import type { PositionedActionMenuState } from "@/shared/ui/actionMenu";
import type { FileResourceRef } from "@/features/application/resource/resourceActions";
import type { FileResourceKind } from "@/shared/types/domain/resource";
export type ProjectSidebarContextMenuTarget =
  | ({ type: "file"; name: string } & FileResourceRef)
  | { type: "fileSection"; kind: FileResourceKind }
  | { type: "dataSection" }
  | { type: "database"; id: string; name: string };
export type ProjectSidebarContextMenuState =
  PositionedActionMenuState<ProjectSidebarContextMenuTarget>;
export interface ProjectSidebarContextMenuActions {
  createFile(kind: FileResourceKind): unknown;
  openFile(ref: FileResourceRef): unknown;
  renameFile(ref: FileResourceRef, name: string): unknown;
  duplicateFile(ref: FileResourceRef): unknown;
  deleteFile(ref: FileResourceRef): unknown;
  openDatabase(id: string): void;
  renameDatabaseItem(id: string, name: string): void;
  deleteDatabaseItem(id: string, name: string): unknown;
  importData(): void;
  revealInExplorer(request: RevealProjectResourceRequest): unknown;
}
