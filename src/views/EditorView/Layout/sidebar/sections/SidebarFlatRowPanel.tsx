import { useMemo, useRef } from 'react';
import { useDetailTarget } from '@/features/core/editor';
import type {
  SidebarPanelModel,
  SidebarSectionActionConfig,
  SidebarSectionKey,
} from '@/features/core/sidebar';
import type { NodeCatalogItem } from '@/features/domain/nodeCatalog';
import type { GraphResourceType } from '../../sidebarContextMenu';
import { SidebarFlatRowContext, type SidebarFlatRowContextValue } from './sidebarFlatRowContext';
import { SidebarFlatRowList } from './SidebarFlatRowList';
import { flattenSidebarPanelModel } from './sidebarRenderRows';

export type SidebarFlatRowPanelProps = {
  model: SidebarPanelModel;
  sectionActions?: Partial<Record<SidebarSectionKey, SidebarSectionActionConfig>>;
  graphIssueCounts?: Record<string, number>;
  onToggleSection: (key: SidebarSectionKey) => void;
  onToggleGroup: (groupKey: string) => void;
  selectedNodeType?: string | null;
  onNodeLeafClick?: (item: NodeCatalogItem) => void;
  onGraphContextMenu?: (
    e: React.MouseEvent,
    target: { type: 'graph'; id: string; name: string; graphType: GraphResourceType },
  ) => void;
  onVariableContextMenu?: (e: React.MouseEvent, id: string, name: string) => void;
  onDatabaseContextMenu?: (e: React.MouseEvent, id: string, name: string) => void;
  onOpenWorksheet?: (id: string, name: string) => void;
  onWorksheetContextMenu?: (e: React.MouseEvent, id: string, name: string) => void;
};

export function SidebarFlatRowPanel({
  model,
  sectionActions = {},
  graphIssueCounts = {},
  onToggleSection,
  onToggleGroup,
  selectedNodeType = null,
  onNodeLeafClick,
  onGraphContextMenu,
  onVariableContextMenu,
  onDatabaseContextMenu,
  onOpenWorksheet,
  onWorksheetContextMenu,
}: SidebarFlatRowPanelProps) {
  const detailTarget = useDetailTarget();
  const rows = useMemo(() => flattenSidebarPanelModel(model), [model]);
  const resolvedSelectedNodeType =
    selectedNodeType ?? (detailTarget?.kind === 'nodeDefinition' ? detailTarget.nodeType : null);

  const handlersRef = useRef<Omit<SidebarFlatRowContextValue, 'detailTarget' | 'graphIssueCounts' | 'selectedNodeType' | 'sectionActions'>>({
    onToggleSection,
    onToggleGroup,
    onNodeLeafClick,
    onGraphContextMenu,
    onVariableContextMenu,
    onDatabaseContextMenu,
    onOpenWorksheet,
    onWorksheetContextMenu,
  });
  handlersRef.current = {
    onToggleSection,
    onToggleGroup,
    onNodeLeafClick,
    onGraphContextMenu,
    onVariableContextMenu,
    onDatabaseContextMenu,
    onOpenWorksheet,
    onWorksheetContextMenu,
  };

  const contextValue = useMemo<SidebarFlatRowContextValue>(
    () => ({
      sectionActions,
      detailTarget,
      graphIssueCounts,
      selectedNodeType: resolvedSelectedNodeType,
      onToggleSection: (key) => handlersRef.current.onToggleSection(key),
      onToggleGroup: (key) => handlersRef.current.onToggleGroup(key),
      onNodeLeafClick: (item) => handlersRef.current.onNodeLeafClick?.(item),
      onGraphContextMenu: (e, target) => handlersRef.current.onGraphContextMenu?.(e, target),
      onVariableContextMenu: (e, id, name) => handlersRef.current.onVariableContextMenu?.(e, id, name),
      onDatabaseContextMenu: (e, id, name) => handlersRef.current.onDatabaseContextMenu?.(e, id, name),
      onOpenWorksheet: (id, name) => handlersRef.current.onOpenWorksheet?.(id, name),
      onWorksheetContextMenu: (e, id, name) => handlersRef.current.onWorksheetContextMenu?.(e, id, name),
    }),
    [detailTarget, graphIssueCounts, resolvedSelectedNodeType, sectionActions],
  );

  return (
    <SidebarFlatRowContext.Provider value={contextValue}>
      <SidebarFlatRowList rows={rows} />
    </SidebarFlatRowContext.Provider>
  );
}
