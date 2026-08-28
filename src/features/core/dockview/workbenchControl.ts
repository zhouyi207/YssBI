import { workbenchDockviewPort } from './workbenchDockviewPort';
import type {
  ConfiguredWorkbenchEdgeState,
  ConfigureWorkbenchEdgeRequest,
  EnsureViewRequest,
  MoveWorkbenchPanelRequest,
  OpenEditorRequest,
  SplitWorkbenchPanelRequest,
  UpsertResultRequest,
  WorkbenchDockviewPort,
} from './workbenchDockviewPort';

export interface WorkbenchDockviewControl {
  ensureCentralGroup(): Promise<string>;
  openEditor(request: OpenEditorRequest): Promise<import('./workbenchDockviewPort').WorkbenchPanelInfo>;
  setEditorPinned(panelInstanceId: string, pinned: boolean): Promise<boolean>;
  ensureView(request: EnsureViewRequest): Promise<import('./workbenchDockviewPort').WorkbenchPanelInfo>;
  upsertResult(request: UpsertResultRequest): Promise<import('./workbenchDockviewPort').WorkbenchPanelInfo>;
  activate(panelInstanceId: string): Promise<boolean>;
  reveal(panelInstanceId: string): Promise<boolean>;
  move(request: MoveWorkbenchPanelRequest): Promise<boolean>;
  split(request: SplitWorkbenchPanelRequest): Promise<boolean>;
  configureEdge(request: ConfigureWorkbenchEdgeRequest): Promise<ConfiguredWorkbenchEdgeState>;
  setEdgeCollapsed(position: import('./workbenchDockviewPort').WorkbenchEdgePosition, collapsed: boolean): Promise<boolean>;
  setEdgeSize(position: import('./workbenchDockviewPort').WorkbenchEdgePosition, size: number): Promise<boolean>;
  remapResource(from: string, to: string): Promise<number>;
  serialize(): Promise<import('dockview-react').SerializedDockview>;
}

export function createWorkbenchDockviewControl(
  port: WorkbenchDockviewPort = workbenchDockviewPort,
): WorkbenchDockviewControl {
  return {
    ensureCentralGroup: port.ensureCentralGroup,
    openEditor: port.openEditor,
    setEditorPinned: port.setEditorPinned,
    ensureView: port.ensureView,
    upsertResult: port.upsertResult,
    activate: port.activate,
    reveal: port.reveal,
    move: port.move,
    split: port.split,
    configureEdge: port.configureEdge,
    setEdgeCollapsed: port.setEdgeCollapsed,
    setEdgeSize: port.setEdgeSize,
    remapResource: port.remapResource,
    serialize: port.serialize,
  };
}

export const workbenchDockviewControl = createWorkbenchDockviewControl();
