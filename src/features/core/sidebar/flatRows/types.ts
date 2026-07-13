import type { HistoryEntry } from '@/features/core/history';
import type { NodeCatalogItem } from '@/features/domain/nodeCatalog';
import type { MouseEvent } from 'react';
import type { SidebarSectionKey } from '../sidebarSectionState';

export const SIDEBAR_FLAT_ROW_HEIGHT = 28 as const;

export type FlatSidebarRow =
  | {
      kind: 'section';
      rowKey: string;
      sectionKey: SidebarSectionKey;
      level: 0;
      label: string;
      expanded: boolean;
    }
  | {
      kind: 'group';
      rowKey: string;
      groupKey: string;
      level: number;
      label: string;
      expanded: boolean;
    }
  | {
      kind: 'empty';
      rowKey: string;
      level: number;
      message: string;
      sectionKey?: SidebarSectionKey;
    }
  | {
      kind: 'graph';
      rowKey: string;
      level: number;
      id: string;
      name: string;
      graphType: 'event' | 'function';
    }
  | {
      kind: 'variable';
      rowKey: string;
      level: number;
      id: string;
      name: string;
      dataType: unknown;
      isGlobal: boolean;
    }
  | {
      kind: 'database';
      rowKey: string;
      level: number;
      id: string;
      name: string;
      data: unknown;
    }
  | {
      kind: 'worksheet';
      rowKey: string;
      level: number;
      id: string;
      name: string;
    }
  | {
      kind: 'history';
      rowKey: string;
      level: number;
      entry: HistoryEntry;
      highlighted: boolean;
      stack: 'undo' | 'redo';
    }
  | {
      kind: 'node';
      rowKey: string;
      level: number;
      item: NodeCatalogItem;
    };

export type SidebarSectionActionConfig = {
  onAdd?: () => void;
  addAriaLabel?: string;
  onHeaderContextMenu?: (e: MouseEvent) => void;
  onContentContextMenu?: (e: MouseEvent) => void;
};
