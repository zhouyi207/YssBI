
import type { Variable, VariableScope } from '@/shared/types/domain';
import { DEFAULT_VARIABLE_NAME } from '@/shared/constants/defaultResourceNames';
import { dataTypeFromKey, getDefaultValue, isVariableDataTypeAllowed } from '@/shared/types/domain/dataType';
import { dataValueFromRaw } from '@/shared/types/domain/dataValue';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useResourceStore } from '@/features/core/resource';
import { ProjectLifecycleError } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { variableCatalogToResourceMetas } from '@/features/core/variable/variableCatalog';
import { VariableService } from '@/services/variable/variableService';
import { isIpcErrorCode } from '@/services/ipc';
import { logger } from '@/utils/appLogger';
import {
  captureRevisionedProjectCommandSnapshot,
  type ProjectCommandContext,
} from '@/features/application/projectCommandContext';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';

function isStaleProjectLifecycleError(error: unknown): boolean {
  return error instanceof ProjectLifecycleError
    || isIpcErrorCode(error, 'stale_project_lifecycle');
}

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
    if (!isVariableDataTypeAllowed(dataType)) return null;
    const snapshot = captureRevisionedProjectCommandSnapshot(
      (): Omit<Variable, 'id' | 'revision'> => ({
        name: baseName,
        dataType,
        dataValue: dataValueFromRaw(getDefaultValue(dataType), dataType),
        description: '',
        scope: buildScope(Boolean(params.isGlobal), params.activeGraphPath, params.graphType),
        tags: [],
      }),
    );
    context = snapshot.context;
    const variable = snapshot.authority;

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
    if (isStaleProjectLifecycleError(e) || (context && !context.isCurrent())) return null;
    logger.data.error('Failed to create variable: ' + String(e), 'VariableActions');
    return null;
  }
}

export async function updateVariableAction(
  id: string,
  data: Partial<Variable>,
): Promise<Variable | null> {
  let context: ProjectCommandContext | undefined;
  try {
    const snapshot = captureRevisionedProjectCommandSnapshot(() => {
      const variableState = useVariableStore.getState();
      return {
        previous: variableState.variables[id],
        expectedRevision: variableState.revisions[id],
      };
    });
    context = snapshot.context;
    const { previous, expectedRevision } = snapshot.authority;
    if (!previous || expectedRevision == null) return null;

    if (data.dataType && !isVariableDataTypeAllowed(data.dataType)) return null;

    const committed = await VariableService.updateVariable(
      context.projectInstanceId,
      context.operationId,
      expectedRevision,
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
    if (isStaleProjectLifecycleError(e) || (context && !context.isCurrent())) return null;
    logger.data.error('Failed to update variable in backend: ' + String(e), 'VariableActions');
    return null;
  }
}

export async function deleteVariableAction(id: string): Promise<boolean> {
  let context: ProjectCommandContext | undefined;
  try {
    const snapshot = captureRevisionedProjectCommandSnapshot(() => {
      const variableState = useVariableStore.getState();
      return {
        previous: variableState.variables[id],
        expectedRevision: variableState.revisions[id],
      };
    });
    context = snapshot.context;
    const { previous, expectedRevision } = snapshot.authority;
    if (!previous || expectedRevision == null) return false;

    const committed = await VariableService.deleteVariable(
      context.projectInstanceId,
      context.operationId,
      expectedRevision,
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
    if (isStaleProjectLifecycleError(e) || (context && !context.isCurrent())) return false;
    logger.data.error('Failed to delete variable in backend: ' + String(e), 'VariableActions');
    return false;
  }
}

export async function renameVariableAction(id: string, name: string): Promise<boolean> {
  const trimmed = name.trim();
  if (!trimmed) return false;
  const result = await updateVariableAction(id, { name: trimmed });
  return result !== null;
}
