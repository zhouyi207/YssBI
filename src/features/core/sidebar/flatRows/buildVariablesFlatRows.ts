import type { VariableListEntry } from '@/features/core/variable/variableScopeSelectors';
import type { FlatSidebarRow } from './types';
import { appendSectionBlock } from './appendSectionBlock';

export function buildVariablesFlatRows(params: {
  localVariables: Record<string, VariableListEntry>;
  globalVariables: Record<string, VariableListEntry>;
  hasActiveGraph: boolean;
  expandedSections: Record<string, boolean>;
  labels: {
    local: string;
    global: string;
    noLocal: string;
    noGlobal: string;
    noActiveGraph: string;
  };
}): FlatSidebarRow[] {
  const rows: FlatSidebarRow[] = [];

  const localItems: FlatSidebarRow[] = params.hasActiveGraph
    ? Object.entries(params.localVariables).map(([id, data]) => ({
        kind: 'variable',
        rowKey: `variable:local:${id}`,
        level: 1,
        id,
        name: data.name,
        dataType: data.dataType,
        isGlobal: false,
      }))
    : [];

  appendSectionBlock(rows, {
    sectionKey: 'variablesLocal',
    label: params.labels.local,
    expandedSections: params.expandedSections,
    emptyMessage: params.hasActiveGraph ? params.labels.noLocal : params.labels.noActiveGraph,
    itemRows: localItems,
  });

  const globalItems: FlatSidebarRow[] = Object.entries(params.globalVariables).map(([id, data]) => ({
    kind: 'variable',
    rowKey: `variable:global:${id}`,
    level: 1,
    id,
    name: data.name,
    dataType: data.dataType,
    isGlobal: true,
  }));

  appendSectionBlock(rows, {
    sectionKey: 'variablesGlobal',
    label: params.labels.global,
    expandedSections: params.expandedSections,
    emptyMessage: params.labels.noGlobal,
    itemRows: globalItems,
  });

  return rows;
}
