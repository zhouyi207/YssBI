import { resolveSectionExpanded } from '../sidebarSectionState';
import type { SidebarPanelModel } from './sidebarPanelModel';
import type { SidebarItemRow } from './types';

export function buildDataSidebarModel(params: {
  dataframes: Record<string, { name: string; resourcePath?: string }>;
  expandedSections: Record<string, boolean>;
  labels: { data: string; noData: string };
}): SidebarPanelModel {
  const items: SidebarItemRow[] = Object.entries(params.dataframes).map(([id, data]) => ({
    kind: 'database',
    rowKey: `database:${id}`,
    level: 1,
    id,
    resourcePath: data.resourcePath,
    name: data.name,
    data,
  }));

  return {
    sections: [
      {
        key: 'dataData',
        label: params.labels.data,
        expanded: resolveSectionExpanded(params.expandedSections, 'dataData'),
        rows: items,
        emptyMessage: params.labels.noData,
      },
    ],
  };
}
