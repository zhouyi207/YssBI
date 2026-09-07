import type { RevealProjectResourceRequest } from "@/features/application/sidebar";
import type { PositionedActionMenuState } from "@/shared/ui/actionMenu";

export type GraphResourceType = "event" | "function";

export type ProjectSidebarContextMenuTarget =
  | { type: "graph"; id: string; name: string; graphType: GraphResourceType }
  | { type: "section"; graphType: GraphResourceType }
  | { type: "chartSection" }
  | { type: "chart"; chartPath: string; name: string };

export type ProjectSidebarContextMenuState =
  PositionedActionMenuState<ProjectSidebarContextMenuTarget>;

export interface ProjectSidebarContextMenuActions {
  openGraph: (id: string, name: string, type: GraphResourceType) => void;
  createGraph: (type: GraphResourceType) => unknown | Promise<unknown>;
  renameGraphItem: (id: string, name: string, type: GraphResourceType) => void;
  deleteGraphItem: (id: string, type: GraphResourceType) => unknown | Promise<unknown>;
  duplicateGraphItem: (id: string) => unknown | Promise<unknown>;
  openChart: (chartPath: string, name: string) => unknown | Promise<unknown>;
  renameChartItem: (chartPath: string, name: string) => void;
  duplicateChart: (chartPath: string) => unknown | Promise<unknown>;
  deleteChart: (chartPath: string) => unknown | Promise<unknown>;
  addChart: () => unknown | Promise<unknown>;
  revealInExplorer: (request: RevealProjectResourceRequest) => unknown | Promise<unknown>;
}
