
import type { NodeCatalogItem } from '@/features/domain/nodeCatalog';
import type { MouseEvent } from 'react';

export const SIDEBAR_FLAT_ROW_HEIGHT = 28 as const;

export interface SidebarGroupItemRow {
  kind: 'group';
  rowKey: string;
  groupKey: string;
  level: number;
  label: string;
  expanded: boolean;
}

export interface SidebarGraphItemRow {
  kind: 'graph';
  rowKey: string;
  level: number;
  id: string;
  name: string;
  graphType: 'event' | 'function';
}

export interface SidebarVariableItemRow {
  kind: 'variable';
  rowKey: string;
  level: number;
  id: string;
  resourcePath?: string;
  name: string;
  dataType: unknown;
  isGlobal: boolean;
}

export interface SidebarDatabaseItemRow {
  kind: 'database';
  rowKey: string;
  level: number;
  id: string;
  resourcePath?: string;
  name: string;
  data: unknown;
}

export interface SidebarWorksheetItemRow {
  kind: 'worksheet';
  rowKey: string;
  level: number;
  id: string;
  name: string;
}

export interface SidebarNodeItemRow {
  kind: 'node';
  rowKey: string;
  level: number;
  item: NodeCatalogItem;
}

export type SidebarItemRow =
  | SidebarGroupItemRow
  | SidebarGraphItemRow
  | SidebarVariableItemRow
  | SidebarDatabaseItemRow
  | SidebarWorksheetItemRow
  | SidebarNodeItemRow;

export type SidebarSectionActionConfig = {
  onAdd?: () => void;
  addAriaLabel?: string;
  onHeaderContextMenu?: (e: MouseEvent) => void;
  onContentContextMenu?: (e: MouseEvent) => void;
};
