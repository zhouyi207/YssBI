import type { Variable, VariableScope } from '@/shared/types/domain';
import { DEFAULT_VARIABLE_NAME } from '@/shared/constants/defaultResourceNames';
import { dataTypeFromKey, getDefaultValue, isVariableDataTypeAllowed } from '@/shared/types/domain/dataType';
import { dataValueFromRaw } from '@/shared/types/domain/dataValue';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useResourceStore } from '@/features/core/resource';
import { variableCatalogToResourceMetas } from '@/features/core/variable/variableCatalog';
import { VariableService } from '@/services/variable/variableService';
import { logger } from '@/utils/appLogger';
import { uiStore } from '@/features/core/ui/UIStore';
import {
  captureProjectCommandContext,
  type ProjectCommandContext,
} from '@/features/application/projectCommandContext';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';

function buildScope(
  isGlobal: boolean,
  activeGraphPath: string | null,
  graphType: 'event' | 'function' | undefined,
): VariableScope {
  if (isGlobal || !activeGraphPath) return { type: 'global' };
  const scopeType = graphType ?? 'event';
  return scopeType === 'function'
    ? { type: 'function', functionPath: activeGraphPath }
    : { type: 'event', eventPath: activeGraphPath };
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
  activeGraphPath: string | null;
  graphType?: 'event' | 'function';
}): Promise<string | null> {
  let context: ProjectCommandContext | undefined;
  try {
    const baseName = params.name || DEFAULT_VARIABLE_NAME;
    const dataType = dataTypeFromKey(params.type ?? 'Int64');
    if (!isVariableDataTypeAllowed(dataType)) {
      uiStore.showToast('变量类型不能为 Any', 'error');
      return null;
    }
    const variable: Omit<Variable, 'id' | 'revision'> = {
      name: baseName,
      dataType,
      dataValue: dataValueFromRaw(getDefaultValue(dataType), dataType),
      description: '',
      scope: buildScope(Boolean(params.isGlobal), params.activeGraphPath, params.graphType),
      tags: [],
    };

    context = captureProjectCommandContext();
    const committed = await VariableService.createVariable(
      context.projectInstanceId,
      context.operationId,
      context.publicationRevision,
      variable,
    );
    if (!context.isCurrent()) return null;
    if (committed.result) {
      await projectPublicationCoordinator.submit({ result: committed.result });
    } else if (committed.variable) {
      useVariableStore.getState().addVariable(committed.variableId, committed.variable);
      rebuildVariableResourceProjection();
    }
    if (!context.isCurrent()) return null;
    return committed.variableId;
  } catch (e) {
    if (context && !context.isCurrent()) return null;
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

  if (data.dataType && !isVariableDataTypeAllowed(data.dataType)) {
    uiStore.showToast('变量类型不能为 Any', 'error');
    return null;
  }

  let context: ProjectCommandContext | undefined;
  try {
    context = captureProjectCommandContext();
    const committed = await VariableService.updateVariable(
      context.projectInstanceId,
      context.operationId,
      previous.revision,
      id,
      data,
    );
    if (!context.isCurrent()) return null;
    if (committed.result) {
      await projectPublicationCoordinator.submit({ result: committed.result });
    } else if (committed.variable) {
      useVariableStore.getState().updateVariable(id, committed.variable);
      rebuildVariableResourceProjection();
    }
    if (!context.isCurrent()) return null;
    return useVariableStore.getState().variables[id] ?? null;
  } catch (e) {
    if (context && !context.isCurrent()) return null;
    logger.data.error('Failed to update variable in backend: ' + String(e), 'VariableActions');
    uiStore.showToast(`变量更新失败: ${e}`, 'error');
    return null;
  }
}

export async function deleteVariableAction(id: string): Promise<boolean> {
  const previous = useVariableStore.getState().variables[id];
  if (!previous) return false;

  let context: ProjectCommandContext | undefined;
  try {
    context = captureProjectCommandContext();
    const committed = await VariableService.deleteVariable(
      context.projectInstanceId,
      context.operationId,
      previous.revision,
      id,
    );
    if (!context.isCurrent()) return false;
    if (committed.result) {
      await projectPublicationCoordinator.submit({ result: committed.result });
    } else {
      useVariableStore.getState().deleteVariable(id);
      useResourceStore.getState().removeResource({ id, kind: 'variable' });
    }
    return context.isCurrent();
  } catch (e) {
    if (context && !context.isCurrent()) return false;
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
