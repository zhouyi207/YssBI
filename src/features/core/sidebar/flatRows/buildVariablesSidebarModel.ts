import type { VariableListEntry } from '@/features/core/variable/variableScopeSelectors';
import { resolveSectionExpanded } from '../sidebarSectionState';
import type { SidebarPanelModel } from './sidebarPanelModel';
import type { SidebarItemRow } from './types';

export function buildVariablesSidebarModel(params: {
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
}): SidebarPanelModel {
  const localItems: SidebarItemRow[] = params.hasActiveGraph
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

  const globalItems: SidebarItemRow[] = Object.entries(params.globalVariables).map(([id, data]) => ({
    kind: 'variable',
    rowKey: `variable:global:${id}`,
    level: 1,
    id,
    name: data.name,
    dataType: data.dataType,
    isGlobal: true,
  }));

  return {
    sections: [
      {
        key: 'variablesLocal',
        label: params.labels.local,
        expanded: resolveSectionExpanded(params.expandedSections, 'variablesLocal'),
        rows: localItems,
        emptyMessage: params.hasActiveGraph ? params.labels.noLocal : params.labels.noActiveGraph,
      },
      {
        key: 'variablesGlobal',
        label: params.labels.global,
        expanded: resolveSectionExpanded(params.expandedSections, 'variablesGlobal'),
        rows: globalItems,
        emptyMessage: params.labels.noGlobal,
      },
    ],
  };
}
