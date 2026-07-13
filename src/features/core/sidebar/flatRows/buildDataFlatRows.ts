import type { FlatSidebarRow } from './types';
import { appendSectionBlock } from './appendSectionBlock';

export function buildDataFlatRows(params: {
  dataframes: Record<string, { name: string }>;
  expandedSections: Record<string, boolean>;
  labels: { data: string; noData: string };
}): FlatSidebarRow[] {
  const rows: FlatSidebarRow[] = [];

  const items: FlatSidebarRow[] = Object.entries(params.dataframes ?? {}).map(([id, data]) => ({
    kind: 'database',
    rowKey: `database:${id}`,
    level: 1,
    id,
    name: data.name,
    data,
  }));

  appendSectionBlock(rows, {
    sectionKey: 'dataData',
    label: params.labels.data,
    expandedSections: params.expandedSections,
    emptyMessage: params.labels.noData,
    itemRows: items,
  });

  return rows;
}
