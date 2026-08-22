import type { RevealProjectResourceRequest } from "@/services/project/projectService";
import type { PositionedActionMenuState } from "@/shared/ui/actionMenu";

export type GraphResourceType = "event" | "function";

export type SidebarContextMenuTarget =
  | { type: "graph"; id: string; name: string; graphType: GraphResourceType }
  | { type: "section"; graphType: GraphResourceType }
  | { type: "variable"; id: string; name: string; isGlobal: boolean }
  | { type: "variableSection"; isGlobal: boolean }
  | { type: "database"; id: string; name: string }
  | { type: "dataSection" }
  | { type: "worksheetSection" }
  | { type: "worksheet"; worksheetPath: string; name: string };

export type SidebarContextMenuState = PositionedActionMenuState<SidebarContextMenuTarget>;

export interface SidebarInputDialogState {
  title: string;
  value: string;
  submitLabel?: string;
  error?: string | null;
  onSubmit: (value: string) => void | Promise<void>;
}

export interface SidebarContextMenuActions {
  openGraph: (id: string, name: string, type: GraphResourceType) => void;
  createGraph: (type: GraphResourceType) => unknown | Promise<unknown>;
  renameGraphItem: (id: string, name: string, type: GraphResourceType) => void;
  deleteGraphItem: (id: string, type: GraphResourceType) => unknown | Promise<unknown>;
  duplicateGraphItem: (id: string) => unknown | Promise<unknown>;
  addVariable: (name: string, dataType: string, isGlobal: boolean) => unknown | Promise<unknown>;
  renameVariableItem: (id: string, name: string) => void;
  deleteVariable: (id: string, name: string) => unknown | Promise<unknown>;
  promoteVariable: (id: string) => unknown | Promise<unknown>;
  demoteVariable: (id: string) => unknown | Promise<unknown>;
  canDemoteVariable: boolean;
  openDatabase: (id: string) => void;
  renameDatabaseItem: (id: string, name: string) => void;
  deleteDatabaseItem: (id: string, name: string) => unknown | Promise<unknown>;
  importData: () => void;
  openWorksheet: (worksheetPath: string, name: string) => unknown | Promise<unknown>;
  renameWorksheetItem: (worksheetPath: string, name: string) => void;
  duplicateWorksheet: (worksheetPath: string) => unknown | Promise<unknown>;
  deleteWorksheet: (worksheetPath: string) => unknown | Promise<unknown>;
  addWorksheet: () => unknown | Promise<unknown>;
  revealInExplorer: (request: RevealProjectResourceRequest) => unknown | Promise<unknown>;
}
