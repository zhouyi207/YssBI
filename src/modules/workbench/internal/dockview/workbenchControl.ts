import { workbenchDockviewRuntime } from "./workbenchDockviewInternal";
import type {
  ConfiguredWorkbenchEdgeState,
  ConfigureWorkbenchEdgeRequest,
  EnsureViewRequest,
  MoveWorkbenchPanelRequest,
  OpenEditorRequest,
  SplitWorkbenchPanelRequest,
  UpsertResultRequest,
  WorkbenchDockviewControlContract,
  WorkbenchEdgePosition,
  WorkbenchPanelInfo,
} from "./workbenchTypes";

export interface WorkbenchDockviewControl {
  ensureCentralGroup(): Promise<string>;
  openEditor(request: OpenEditorRequest): Promise<WorkbenchPanelInfo>;
  setEditorPinned(panelInstanceId: string, pinned: boolean): Promise<boolean>;
  ensureView(request: EnsureViewRequest): Promise<WorkbenchPanelInfo>;
  upsertResult(request: UpsertResultRequest): Promise<WorkbenchPanelInfo>;
  activate(panelInstanceId: string): Promise<boolean>;
  reveal(panelInstanceId: string): Promise<boolean>;
  move(request: MoveWorkbenchPanelRequest): Promise<boolean>;
  split(request: SplitWorkbenchPanelRequest): Promise<boolean>;
  configureEdge(request: ConfigureWorkbenchEdgeRequest): Promise<ConfiguredWorkbenchEdgeState>;
  setEdgeCollapsed(position: WorkbenchEdgePosition, collapsed: boolean): Promise<boolean>;
  setEdgeSize(position: WorkbenchEdgePosition, size: number): Promise<boolean>;
  remapResource(from: string, to: string): Promise<number>;
  serialize(): Promise<import("dockview-react").SerializedDockview>;
}

export const workbenchDockviewControl: WorkbenchDockviewControl = workbenchDockviewRuntime.control;

export type WorkbenchDockviewControlContractType = WorkbenchDockviewControlContract;
