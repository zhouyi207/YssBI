import { resolveSectionExpanded } from '../sidebarSectionState';
import type { SidebarPanelModel } from './sidebarPanelModel';
import type { SidebarItemRow } from './types';

type GraphRecord = Record<string, { name: string }>;

export function buildGraphsSidebarModel(params: {
  events: GraphRecord;
  functions: GraphRecord;
  expandedSections: Record<string, boolean>;
  labels: {
    event: string;
    function: string;
    noEvents: string;
    noFunctions: string;
  };
}): SidebarPanelModel {
  const eventItems: SidebarItemRow[] = Object.entries(params.events).map(([id, data]) => ({
    kind: 'graph',
    rowKey: `graph:event:${id}`,
    level: 1,
    id,
    name: data.name,
    graphType: 'event',
  }));

  const functionItems: SidebarItemRow[] = Object.entries(params.functions).map(([id, data]) => ({
    kind: 'graph',
    rowKey: `graph:function:${id}`,
    level: 1,
    id,
    name: data.name,
    graphType: 'function',
  }));

  return {
    sections: [
      {
        key: 'graphsEvent',
        label: params.labels.event,
        expanded: resolveSectionExpanded(params.expandedSections, 'graphsEvent'),
        rows: eventItems,
        emptyMessage: params.labels.noEvents,
      },
      {
        key: 'graphsFunction',
        label: params.labels.function,
        expanded: resolveSectionExpanded(params.expandedSections, 'graphsFunction'),
        rows: functionItems,
        emptyMessage: params.labels.noFunctions,
      },
    ],
  };
}
