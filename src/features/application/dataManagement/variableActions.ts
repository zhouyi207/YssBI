import type { Variable, VariableScope } from '@/shared/types/domain';
import { DEFAULT_VARIABLE_NAME } from '@/shared/constants/defaultResourceNames';
import { dataTypeFromKey, getDefaultValue } from '@/shared/types/domain/dataType';
import { dataValueFromRaw } from '@/shared/types/domain/dataValue';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useResourceStore } from '@/features/core/resource';
import { variableCatalogToResourceMetas } from '@/features/core/variable/variableCatalog';
import { VariableService } from '@/services/variable/variableService';
import { logger } from '@/utils/appLogger';
import { uiStore } from '@/features/core/ui/UIStore';

function buildScope(
  isGlobal: boolean,
  activeGraphId: string | null,
  graphType: 'event' | 'function' | undefined,
): VariableScope {
  if (isGlobal || !activeGraphId) return { type: 'global' };
  const scopeType = graphType ?? 'event';
  return scopeType === 'function'
    ? { type: 'function', functionId: activeGraphId }
    : { type: 'event', eventId: activeGraphId };
}

export function rebuildVariableResourceProjection(): void {
  const variables = useVariableStore.getState().variables;
  const metas = variableCatalogToResourceMetas(variables);
  const store = useResourceStore.getState();
  const nextIds = new Set(Object.keys(variables));
  for (const meta of metas) {
    store.upsertResource(meta);
  }
  for (const key of Object.keys(store.resources)) {
    if (!key.startsWith('variable:')) continue;
    const id = key.slice('variable:'.length);
    if (!nextIds.has(id)) {
      store.removeResource({ id, kind: 'variable' });
    }
  }
}

export async function createVariableAction(params: {
  name?: string;
  type?: string;
  isGlobal?: boolean;
  activeGraphId: string | null;
  graphType?: 'event' | 'function';
}): Promise<string | null> {
  try {
    const baseName = params.name || DEFAULT_VARIABLE_NAME;
    const dataType = dataTypeFromKey(params.type ?? 'Int32');
    const variable: Omit<Variable, 'id'> = {
      name: baseName,
      dataType,
      dataValue: dataValueFromRaw(getDefaultValue(dataType), dataType),
      description: '',
      scope: buildScope(Boolean(params.isGlobal), params.activeGraphId, params.graphType),
      tags: [],
    };

    const newVarId = await VariableService.createVariable(variable);
    const newVar = await VariableService.getVariable(newVarId);
    useVariableStore.getState().addVariable(newVarId, newVar);
    rebuildVariableResourceProjection();
    return newVarId;
  } catch (e) {
    logger.data.error('Failed to create variable: ' + String(e), 'VariableActions');
    uiStore.showToast(`变量创建失败: ${e}`, 'error');
    return null;
  }
}

export async function updateVariableAction(
  id: string,
  data: Partial<Variable>,
): Promise<Variable | null> {
  const previous = useVariableStore.getState().variables[id];
  if (!previous) return null;

  try {
    const next = await VariableService.updateVariable(id, data);
    useVariableStore.getState().updateVariable(id, next);
    rebuildVariableResourceProjection();
    return next;
  } catch (e) {
    logger.data.error('Failed to update variable in backend: ' + String(e), 'VariableActions');
    uiStore.showToast(`变量更新失败: ${e}`, 'error');
    return null;
  }
}

export async function deleteVariableAction(id: string): Promise<boolean> {
  const previous = useVariableStore.getState().variables[id];
  if (!previous) return false;

  try {
    await VariableService.deleteVariable(id);
    useVariableStore.getState().deleteVariable(id);
    useResourceStore.getState().removeResource({ id, kind: 'variable' });
    return true;
  } catch (e) {
    logger.data.error('Failed to delete variable in backend: ' + String(e), 'VariableActions');
    uiStore.showToast(`变量删除失败: ${e}`, 'error');
    return false;
  }
}

export async function renameVariableAction(id: string, name: string): Promise<boolean> {
  const trimmed = name.trim();
  if (!trimmed) return false;
  const result = await updateVariableAction(id, { name: trimmed });
  return result !== null;
}
