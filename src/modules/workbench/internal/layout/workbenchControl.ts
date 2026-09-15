import { workbenchLayoutRuntime } from "./workbenchLayoutInternal";
import type {
  ConfiguredWorkbenchEdgeState,
  ConfigureWorkbenchEdgeRequest,
  EnsureViewRequest,
  MoveWorkbenchPanelRequest,
  OpenEditorRequest,
  SplitWorkbenchPanelRequest,
  UpsertResultRequest,
  WorkbenchLayoutControlContract,
  WorkbenchEdgePosition,
  WorkbenchPanelInfo,
} from "./workbenchTypes";

export interface WorkbenchLayoutControl {
  ensureCentralGroup(): Promise<string>;
  openEditor(request: OpenEditorRequest): Promise<WorkbenchPanelInfo>;
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
  serialize(): Promise<import("flexlayout-react").IJsonModel>;
}

export const workbenchLayoutControl: WorkbenchLayoutControl = workbenchLayoutRuntime.control;

export type WorkbenchLayoutControlContractType = WorkbenchLayoutControlContract;
