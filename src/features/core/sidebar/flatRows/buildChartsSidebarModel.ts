import { resolveSectionExpanded } from '../sidebarSectionState';
import type { SidebarPanelModel } from './sidebarPanelModel';
import type { SidebarItemRow } from './types';

export function buildChartsSidebarModel(params: {
  worksheets: ReadonlyArray<{ worksheetPath: string; name: string }>;
  expandedSections: Record<string, boolean>;
  labels: { worksheets: string; noWorksheets: string };
}): SidebarPanelModel {
  const items: SidebarItemRow[] = params.worksheets.map((ws) => ({
    kind: 'worksheet',
    rowKey: `worksheet:${ws.worksheetPath}`,
    level: 1,
    worksheetPath: ws.worksheetPath,
    name: ws.name,
  }));

  return {
    sections: [
      {
        key: 'chartsWorksheets',
        label: params.labels.worksheets,
        expanded: resolveSectionExpanded(params.expandedSections, 'chartsWorksheets'),
        rows: items,
        emptyMessage: params.labels.noWorksheets,
      },
    ],
  };
}
