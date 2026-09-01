import type { DeepReadonly } from "@/shared/types/deepReadonly";

import { workbenchDockviewRuntime } from "./workbenchDockviewInternal";
import type {
  WorkbenchEdgePosition,
  WorkbenchEdgeState,
  WorkbenchGroupInfo,
  WorkbenchPanelInfo,
} from "./workbenchTypes";

export interface WorkbenchDockviewRead {
  readonly isReady: boolean;
  readonly isHydrated: boolean;
  whenHydrated(): Promise<{ readonly status: "hydrated" | "unbound" }>;
  subscribe(listener: () => void): () => void;
  getSnapshot(): DeepReadonly<{ revision: number; ready: boolean; hydrated: boolean }>;
  getPanel(panelInstanceId: string): DeepReadonly<WorkbenchPanelInfo> | undefined;
  getActivePanel(): DeepReadonly<WorkbenchPanelInfo> | undefined;
  getActiveEditorPanel(): DeepReadonly<WorkbenchPanelInfo> | undefined;
  listPanels(): readonly DeepReadonly<WorkbenchPanelInfo>[];
  listGroups(): readonly DeepReadonly<WorkbenchGroupInfo>[];
  listGroupPanels(groupId: string): readonly DeepReadonly<WorkbenchPanelInfo>[];
  findEditorPanelsByResource(resourceRef: string): readonly DeepReadonly<WorkbenchPanelInfo>[];
  getEdgeState(position: WorkbenchEdgePosition): DeepReadonly<WorkbenchEdgeState>;
}

export const workbenchDockviewRead: WorkbenchDockviewRead = workbenchDockviewRuntime.read;

export type { WorkbenchEdgePosition, WorkbenchEdgeState, WorkbenchGroupInfo, WorkbenchPanelInfo };
