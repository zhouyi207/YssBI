import type { ProjectVariableIndexRow } from '@/services/project/projectService';
import type { Variable } from '@/shared/types';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';
import type { ProjectResourceMeta } from '@/features/core/resource';

export function variableFromIndexRow(row: ProjectVariableIndexRow): Variable {
  return normalizeVariableFromBackend({
    id: row.id,
    revision: row.revision,
    name: row.name,
    dataType: row.dataType,
    dataValue: row.dataValue,
    description: row.description ?? '',
    scope: row.scope,
    tags: row.tags ?? [],
  });
}

export function applyVariableCatalogFromIndex(
  rows: ProjectVariableIndexRow[] | undefined,
): Record<string, Variable> {
  const catalog: Record<string, Variable> = {};
  for (const row of rows ?? []) {
    catalog[row.id] = variableFromIndexRow(row);
  }
  return catalog;
}

export function variableCatalogToResourceMetas(
  variables: Record<string, Variable>,
): ProjectResourceMeta[] {
  return Object.entries(variables).map(([id, variable]) => {
    const scope =
      variable.scope.type === 'event'
        ? { type: 'event' as const , graphPath: variable.scope.eventPath }
        : variable.scope.type === 'function'
          ? { type: 'function' as const , graphPath: variable.scope.functionPath }
          : { type: 'global' as const };
    return {
      id,
      kind: 'variable',
      name: variable.name,
      uri: `yssbi://variable/${id}`,
      scope,
      exists: true,
      loaded: true,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    };
  });
}
