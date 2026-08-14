import type { SerializedDockview } from 'dockview-react';

export type PanelInstanceId = string;
export type ResourceRef = string;
export type SplitDirection = 'top' | 'bottom' | 'left' | 'right';

/** Serializable editor identity carried by a Dockview panel, not by its id. */
export interface LayoutTabMetadata {
  readonly resourceRef: ResourceRef;
  readonly kind: string;
  readonly data?: Readonly<Record<string, unknown>>;
}

export interface DockviewPanelParams extends Record<string, unknown> {
  readonly layoutTab: LayoutTabMetadata;
}

export interface OpenPanelRequest {
  readonly panelInstanceId: PanelInstanceId;
  readonly component: string;
  readonly tab: LayoutTabMetadata;
  readonly title?: string;
  readonly tabComponent?: string;
  readonly params?: Readonly<Record<string, unknown>>;
  readonly groupId?: string;
  readonly index?: number;
  readonly inactive?: boolean;
}

export interface MovePanelRequest {
  readonly panelInstanceId: PanelInstanceId;
  readonly groupId: string;
  readonly index?: number;
  readonly activate?: boolean;
}

export interface SplitPanelRequest {
  readonly panelInstanceId: PanelInstanceId;
  readonly referenceGroupId: string;
  readonly direction: SplitDirection;
  readonly activate?: boolean;
}

export interface DockviewPanelInfo {
  readonly panelInstanceId: PanelInstanceId;
  readonly groupId: string;
  readonly component: string;
  readonly title?: string;
  readonly tab?: LayoutTabMetadata;
  readonly active: boolean;
}

export interface DockviewGroupInfo {
  readonly groupId: string;
  readonly panelInstanceIds: readonly PanelInstanceId[];
  readonly activePanelInstanceId?: PanelInstanceId;
  readonly active: boolean;
}

export interface DockviewPortSnapshot {
  readonly revision: number;
  readonly ready: boolean;
}

export type DockviewLayout = SerializedDockview;
