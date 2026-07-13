import type { FlatSidebarRow } from './types';
import { appendSectionBlock } from './appendSectionBlock';

type GraphRecord = Record<string, { name: string }>;

export function buildGraphsFlatRows(params: {
  events: GraphRecord;
  functions: GraphRecord;
  expandedSections: Record<string, boolean>;
  labels: {
    event: string;
    function: string;
    noEvents: string;
    noFunctions: string;
  };
}): FlatSidebarRow[] {
  const rows: FlatSidebarRow[] = [];

  const eventItems: FlatSidebarRow[] = Object.entries(params.events).map(([id, data]) => ({
    kind: 'graph',
    rowKey: `graph:event:${id}`,
    level: 1,
    id,
    name: data.name,
    graphType: 'event',
  }));

  appendSectionBlock(rows, {
    sectionKey: 'graphsEvent',
    label: params.labels.event,
    expandedSections: params.expandedSections,
    emptyMessage: params.labels.noEvents,
    itemRows: eventItems,
  });

  const functionItems: FlatSidebarRow[] = Object.entries(params.functions).map(([id, data]) => ({
    kind: 'graph',
    rowKey: `graph:function:${id}`,
    level: 1,
    id,
    name: data.name,
    graphType: 'function',
  }));

  appendSectionBlock(rows, {
    sectionKey: 'graphsFunction',
    label: params.labels.function,
    expandedSections: params.expandedSections,
    emptyMessage: params.labels.noFunctions,
    itemRows: functionItems,
  });

  return rows;
}
