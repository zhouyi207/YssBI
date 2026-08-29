import type { SerializedDockview } from 'dockview-react';

import type {
  EditorResourceKind,
  ResultPanelMetadata,
  WorkbenchComponentId,
  WorkbenchPanelMetadata,
  WorkbenchViewId,
} from './workbenchPanelModel';

export type WorkbenchEdgePosition = 'top' | 'bottom' | 'left' | 'right';

export interface WorkbenchPanelInfo {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly component: WorkbenchComponentId;
  readonly title?: string;
  readonly metadata: WorkbenchPanelMetadata;
  readonly active: boolean;
  /** Live Dockview visibility; omitted by transaction-only projections. */
  readonly visible?: boolean;
  readonly location:
    | { readonly type: 'grid' }
    | { readonly type: 'edge'; readonly position: WorkbenchEdgePosition };
}

export interface WorkbenchGroupInfo {
  readonly groupId: string;
  readonly panelInstanceIds: readonly string[];
  readonly activePanelInstanceId?: string;
  readonly active: boolean;
  readonly location:
    | { readonly type: 'grid' }
    | { readonly type: 'edge'; readonly position: WorkbenchEdgePosition };
}

export interface WorkbenchEdgeState {
  readonly position: WorkbenchEdgePosition;
  readonly exists: boolean;
  readonly groupId?: string;
  readonly visible: boolean;
  readonly collapsed: boolean;
  readonly size?: number;
}

export interface ConfiguredWorkbenchEdgeState extends WorkbenchEdgeState {
  readonly exists: true;
  readonly groupId: string;
}

export interface WorkbenchPanelCommitToken {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly metadata: WorkbenchPanelMetadata;
}

export interface MoveWorkbenchPanelRequest {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly index?: number;
  readonly activate?: boolean;
}

export interface SplitWorkbenchPanelRequest {
  readonly panelInstanceId: string;
  readonly referenceGroupId: string;
  readonly direction: 'top' | 'bottom' | 'left' | 'right';
  readonly activate?: boolean;
}

export interface ConfigureWorkbenchEdgeRequest {
  readonly position: WorkbenchEdgePosition;
  readonly size: number;
  readonly collapsed: boolean;
  readonly headerPosition?: 'top' | 'bottom' | 'left' | 'right';
}

export interface OpenEditorRequest {
  readonly resourceRef: string;
  readonly resourceKind: EditorResourceKind;
  readonly title: string;
  readonly pinned: boolean;
  readonly sticky?: boolean;
  readonly targetGroupId?: string;
  readonly index?: number;
  readonly mode: 'reuse-resource' | 'new-instance';
}

export interface EnsureViewRequest {
  readonly viewId: WorkbenchViewId;
  readonly title: string;
}

export type UpsertResultRequest = Omit<ResultPanelMetadata, 'role'>;

export type WorkbenchLayoutErrorCode =
  | 'dockview_not_ready'
  | 'invalid_panel_metadata'
  | 'group_not_found'
  | 'panel_open_failed'
  | 'layout_restore_failed';

export class WorkbenchLayoutError extends Error {
  constructor(
    readonly code: WorkbenchLayoutErrorCode,
    readonly details: Readonly<Record<string, string>> = {},
  ) {
    super(code);
    this.name = 'WorkbenchLayoutError';
  }
}

export interface WorkbenchDockviewReadContract {
  readonly isReady: boolean;
  readonly isHydrated: boolean;
  whenHydrated(): Promise<{ readonly status: 'hydrated' | 'unbound' }>;
  subscribe(listener: () => void): () => void;
  getSnapshot(): Readonly<{ revision: number; ready: boolean; hydrated: boolean }>;
  getPanel(panelInstanceId: string): WorkbenchPanelInfo | undefined;
  getActivePanel(): WorkbenchPanelInfo | undefined;
  getActiveEditorPanel(): WorkbenchPanelInfo | undefined;
  listPanels(): readonly WorkbenchPanelInfo[];
  listGroups(): readonly WorkbenchGroupInfo[];
  listGroupPanels(groupId: string): readonly WorkbenchPanelInfo[];
  findEditorPanelsByResource(resourceRef: string): readonly WorkbenchPanelInfo[];
  getEdgeState(position: WorkbenchEdgePosition): WorkbenchEdgeState;
}

export interface WorkbenchDockviewControlContract {
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
  serialize(): Promise<SerializedDockview>;
}
