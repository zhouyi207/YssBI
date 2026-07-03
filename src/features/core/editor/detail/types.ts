import type { LayoutTab } from '@/shared/types/layout/layout';
import type { LogMessage } from '@/shared/types/ui';

export type SidebarDetailFocusType = 'variable' | 'data';

export interface SidebarDetailFocus {
  id: string;
  type: SidebarDetailFocusType;
}

export type DetailTarget =
  | { kind: 'node'; id: string; graphId: string }
  | { kind: 'variable'; id: string }
  | { kind: 'data'; id: string }
  | { kind: 'log' }
  | { kind: 'event'; id: string }
  | { kind: 'function'; id: string }
  | { kind: 'worksheet'; id: string };

export interface DetailTargetInput {
  activeTabId: string | null;
  tabs: LayoutTab[];
  selectedNodeIds: string[];
  sidebarDetailFocus: SidebarDetailFocus | null;
  selectedLog: LogMessage | null;
}
