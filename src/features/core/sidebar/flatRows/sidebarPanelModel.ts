import type { SidebarSectionKey } from '../sidebarSectionState';
import type { SidebarItemRow } from './types';

export interface SidebarEmptyStateModel {
  title: string;
  description?: string;
  action?: {
    label: string;
    command: string;
  };
}

export interface SidebarSectionModel {
  key: SidebarSectionKey;
  label: string;
  expanded: boolean;
  rows: SidebarItemRow[];
  emptyMessage?: string;
}

export interface SidebarPanelModel {
  sections: SidebarSectionModel[];
  emptyState?: SidebarEmptyStateModel;
}

export interface SidebarTreeModel {
  rows: SidebarItemRow[];
  emptyState?: SidebarEmptyStateModel;
}
