import type { FlatSidebarRow } from './types';
import { appendSectionBlock } from './appendSectionBlock';

export function buildChartsFlatRows(params: {
  worksheets: ReadonlyArray<{ id: string; name: string }>;
  expandedSections: Record<string, boolean>;
  labels: { worksheets: string; noWorksheets: string };
}): FlatSidebarRow[] {
  const rows: FlatSidebarRow[] = [];

  const items: FlatSidebarRow[] = params.worksheets.map((ws) => ({
    kind: 'worksheet',
    rowKey: `worksheet:${ws.id}`,
    level: 1,
    id: ws.id,
    name: ws.name,
  }));

  appendSectionBlock(rows, {
    sectionKey: 'chartsWorksheets',
    label: params.labels.worksheets,
    expandedSections: params.expandedSections,
    emptyMessage: params.labels.noWorksheets,
    itemRows: items,
  });

  return rows;
}
