import type { RevealProjectResourceRequest } from "@/features/application/sidebar";
import type { PositionedActionMenuState } from "@/shared/ui/actionMenu";

export type DataSidebarContextMenuTarget =
  | { type: "database"; id: string; name: string }
  | { type: "dataSection" };

export type DataSidebarContextMenuState = PositionedActionMenuState<DataSidebarContextMenuTarget>;

export interface DataSidebarContextMenuActions {
  readonly openDatabase: (id: string) => void;
  readonly renameDatabaseItem: (id: string, name: string) => void;
  readonly deleteDatabaseItem: (id: string, name: string) => unknown | Promise<unknown>;
  readonly importData: () => void;
  readonly revealInExplorer: (request: RevealProjectResourceRequest) => unknown | Promise<unknown>;
}
