import { createContext, useContext } from 'react';
import type { DetailTarget } from '@/features/core/editor/detail/types';
import type { SidebarSectionActionConfig, SidebarSectionKey } from '@/features/core/sidebar';

export type SidebarFlatRowContextValue = {
  sectionActions: Partial<Record<SidebarSectionKey, SidebarSectionActionConfig>>;
  detailTarget: DetailTarget | null;
  onToggleSection: (key: SidebarSectionKey) => void;
  onDatabaseContextMenu?: (e: React.MouseEvent, id: string, name: string) => void;
};

export const SidebarFlatRowContext = createContext<SidebarFlatRowContextValue | null>(null);

export function useSidebarFlatRowContext(): SidebarFlatRowContextValue {
  const ctx = useContext(SidebarFlatRowContext);
  if (!ctx) {
    throw new Error('useSidebarFlatRowContext must be used within SidebarFlatRowPanel');
  }
  return ctx;
}
