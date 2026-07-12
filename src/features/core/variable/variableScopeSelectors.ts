import { dataTypeKind } from '@/shared/types/domain/dataType';
import type { DataType } from '@/shared/types/domain/dataType';
import type { Variable } from '@/shared/types/domain/variable';
import { variableVisibleInGraph } from '@/shared/types/domain/variable';

export interface VariableListEntry {
  id: string;
  name: string;
  typeLabel: string;
  dataType: DataType;
}

function toListEntry(variable: Variable): VariableListEntry {
  return {
    id: variable.id,
    name: variable.name,
    typeLabel: dataTypeKind(variable.dataType),
    dataType: variable.dataType,
  };
}

export function selectGlobalVariableEntries(
  variables: Record<string, Variable>,
): Record<string, VariableListEntry> {
  const result: Record<string, VariableListEntry> = {};
  for (const variable of Object.values(variables)) {
    if (variable.scope.type !== 'global') continue;
    result[variable.id] = toListEntry(variable);
  }
  return result;
}

export function selectLocalVariableEntriesForGraph(
  variables: Record<string, Variable>,
  graphPath: string,
  graphKind: 'event' | 'function',
): VariableListEntry[] {
  const entries: VariableListEntry[] = [];
  for (const variable of Object.values(variables)) {
    if (variable.scope.type === 'global') continue;
    if (!variableVisibleInGraph(variable.scope, graphPath, graphKind)) continue;
    entries.push(toListEntry(variable));
  }
  return entries.sort((a, b) => a.name.localeCompare(b.name));
}

export function partitionVariableCatalog(
  variables: Record<string, Variable>,
  graphScope?: { graphPath: string; graphKind: 'event' | 'function' },
): { global: Record<string, VariableListEntry>; local: Record<string, VariableListEntry> } {
  const global = selectGlobalVariableEntries(variables);
  const local: Record<string, VariableListEntry> = {};

  if (graphScope) {
    for (const entry of selectLocalVariableEntriesForGraph(
      variables,
      graphScope.graphPath,
      graphScope.graphKind,
    )) {
      local[entry.id] = entry;
    }
  }

  return { global, local };
}
