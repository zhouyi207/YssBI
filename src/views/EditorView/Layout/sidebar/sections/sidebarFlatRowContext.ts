import { createContext, useContext } from 'react';
import type { NodeCatalogItem } from '@/features/domain/nodeCatalog';
import type { DetailTarget } from '@/features/core/editor/detail/types';
import type { SidebarSectionActionConfig, SidebarSectionKey } from '@/features/core/sidebar';
import type { GraphResourceType } from '../../sidebarContextMenu';

export type SidebarFlatRowContextValue = {
  sectionActions: Partial<Record<SidebarSectionKey, SidebarSectionActionConfig>>;
  detailTarget: DetailTarget | null;
  graphIssueCounts: Record<string, number>;
  selectedNodeType: string | null;
  onToggleSection: (key: SidebarSectionKey) => void;
  onToggleGroup: (groupKey: string) => void;
  onNodeLeafClick?: (item: NodeCatalogItem) => void;
  onGraphContextMenu?: (
    e: React.MouseEvent,
    target: { type: 'graph'; id: string; name: string; graphType: GraphResourceType },
  ) => void;
  onVariableContextMenu?: (e: React.MouseEvent, id: string, name: string) => void;
  onDatabaseContextMenu?: (e: React.MouseEvent, id: string, name: string) => void;
  onOpenWorksheet?: (worksheetPath: string, name: string) => void;
  onWorksheetContextMenu?: (
    e: React.MouseEvent,
    worksheetPath: string,
    name: string,
  ) => void;
};

export const SidebarFlatRowContext = createContext<SidebarFlatRowContextValue | null>(null);

export function useSidebarFlatRowContext(): SidebarFlatRowContextValue {
  const ctx = useContext(SidebarFlatRowContext);
  if (!ctx) {
    throw new Error('useSidebarFlatRowContext must be used within SidebarFlatRowPanel');
  }
  return ctx;
}

/** Stable no-op for tabs without node groups. */
export const noopSidebarHandler = () => undefined;
