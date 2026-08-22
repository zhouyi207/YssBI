import type { SidebarSectionKey } from '../sidebarSectionState';
import type { SidebarItemRow } from './types';

export interface SidebarSectionModel {
  key: SidebarSectionKey;
  label: string;
  expanded: boolean;
  rows: SidebarItemRow[];
  emptyMessage?: string;
}

export interface SidebarPanelModel {
  sections: SidebarSectionModel[];
}
