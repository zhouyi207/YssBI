import type { RevealProjectResourceRequest } from "@/services/project/projectService";
import type { ContextMenuSection, PositionedContextMenuState } from "@/shared/ui/contextMenu";
import type { TFunction } from "i18next";

export type GraphResourceType = "event" | "function";

export type SidebarContextMenuTarget =
  | { type: "graph"; id: string; name: string; graphType: GraphResourceType }
  | { type: "section"; graphType: GraphResourceType }
  | { type: "variable"; id: string; name: string }
  | { type: "variableSection"; isGlobal: boolean }
  | { type: "database"; id: string; name: string }
  | { type: "dataSection" }
  | { type: "worksheetSection" }
  | { type: "worksheet"; id: string; name: string };

export type SidebarContextMenuState = PositionedContextMenuState<SidebarContextMenuTarget>;

export interface SidebarInputDialogState {
  title: string;
  value: string;
  submitLabel?: string;
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
  deleteVariable: (id: string) => unknown | Promise<unknown>;
  openDatabase: (id: string) => void;
  renameDatabaseItem: (id: string, name: string) => void;
  deleteDatabaseItem: (id: string) => unknown | Promise<unknown>;
  importData: () => void;
  openWorksheet: (id: string, name: string) => unknown | Promise<unknown>;
  renameWorksheet: (id: string, name: string) => void;
  deleteWorksheet: (id: string) => unknown | Promise<unknown>;
  addWorksheet: () => unknown | Promise<unknown>;
  revealInExplorer: (request: RevealProjectResourceRequest) => unknown | Promise<unknown>;
}

export type SidebarContextMenuSectionsBuilder = (
  contextMenu: SidebarContextMenuState | null,
  actions: SidebarContextMenuActions,
  t: TFunction
) => ContextMenuSection[];
