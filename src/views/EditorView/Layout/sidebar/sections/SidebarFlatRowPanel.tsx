import { useMemo, useRef } from 'react';
import { useEditorUi } from '@/features/core/editor/ui';
import type { SidebarPanelModel } from '@/features/core/sidebar/flatRows';
import type { SidebarSectionActionConfig, SidebarSectionKey } from '@/features/core/sidebar';
import { SidebarFlatRowContext, type SidebarFlatRowContextValue } from './sidebarFlatRowContext';
import { SidebarFlatRowList } from './SidebarFlatRowList';
import { flattenSidebarPanelModel } from './sidebarRenderRows';

export type SidebarFlatRowPanelProps = {
  model: SidebarPanelModel;
  sectionActions?: Partial<Record<SidebarSectionKey, SidebarSectionActionConfig>>;
  onToggleSection: (key: SidebarSectionKey) => void;
  onDatabaseContextMenu?: (e: React.MouseEvent, id: string, name: string) => void;
};

export function SidebarFlatRowPanel({
  model,
  sectionActions = {},
  onToggleSection,
  onDatabaseContextMenu,
}: SidebarFlatRowPanelProps) {
  const detailTarget = useEditorUi((snapshot) => snapshot.detailFocus);
  const rows = useMemo(() => flattenSidebarPanelModel(model), [model]);

  const handlersRef = useRef<Omit<SidebarFlatRowContextValue, 'detailTarget' | 'sectionActions'>>({
    onToggleSection,
    onDatabaseContextMenu,
  });
  handlersRef.current = {
    onToggleSection,
    onDatabaseContextMenu,
  };

  const contextValue = useMemo<SidebarFlatRowContextValue>(
    () => ({
      sectionActions,
      detailTarget,
      onToggleSection: (key) => handlersRef.current.onToggleSection(key),
      onDatabaseContextMenu: (e, id, name) => handlersRef.current.onDatabaseContextMenu?.(e, id, name),
    }),
    [detailTarget, sectionActions],
  );

  return (
    <SidebarFlatRowContext.Provider value={contextValue}>
      <SidebarFlatRowList rows={rows} />
    </SidebarFlatRowContext.Provider>
  );
}
