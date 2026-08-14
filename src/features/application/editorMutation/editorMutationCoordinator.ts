import { markResourceStale } from '@/features/core/resource';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import type {
  EditorGraphMutationDto,
  GraphMutationResultDto,
  HistoryStatusDto,
  MutationRequestDto,
} from '@/shared/types/dto/editorMutation';
import { GraphMutationService } from '@/services/nodeSystem/graphMutationService';
import { hydrateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { getGraphProjectionBasis } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { applyMutationResult, validateMutationResult } from './applyMutationResult';
import { setHistoryStatus } from './historyCoordinator';
import {
  graphMutationErrorCode,
  type GraphMutationRejectionCode,
} from './graphMutationError';
import {
  completePendingMutation,
  registerPendingMutation,
  resetPendingMutations,
  type PendingMutationRecord,
} from './pendingMutationRegistry';

export interface ExecuteEditorMutationInput {
  graphPath: string;
  locale: string;
  mutation: EditorGraphMutationDto;
}

export interface EditorMutationCoordinatorDependencies {
  createOperationId(): string;
  mutateGraph(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    request: MutationRequestDto<EditorGraphMutationDto>,
  ): Promise<GraphMutationResultDto>;
  hydrateGraph(graphPath: string, locale: string): Promise<unknown>;
  updateHistoryStatus(status: HistoryStatusDto): void;
}

export type ExecuteEditorMutationOutcome =
  | { status: 'applied'; result: GraphMutationResultDto }
  | { status: 'noop'; result: GraphMutationResultDto }
  | { status: 'stale'; result?: GraphMutationResultDto }
  | { status: 'conflict' }
  | { status: 'rejected'; code: GraphMutationRejectionCode };



function defaultOperationId(): string {
  return crypto.randomUUID();
}



const defaultDependencies: EditorMutationCoordinatorDependencies = {
  createOperationId: defaultOperationId,
  mutateGraph: (projectInstanceId, graphPath, locale, request) =>
    GraphMutationService.mutateGraph(projectInstanceId, graphPath, locale, request),
  hydrateGraph: hydrateGraphProjection,
  updateHistoryStatus: setHistoryStatus,
};

function hasErrorCode(error: unknown, code: string): boolean {
  return typeof error === 'object'
    && error !== null
    && 'code' in error
    && (error as { code?: unknown }).code === code;
}

function isRevisionConflict(error: unknown): boolean {
  return hasErrorCode(error, 'graph_revision_conflict');
}

async function requestAuthoritativeHydrate(
  graphPath: string,
  locale: string,
  dependencies: EditorMutationCoordinatorDependencies,
): Promise<void> {
  const kind = inferGraphResourceKind(graphPath);
  if (kind) markResourceStale({ id: graphPath, kind }, true);
  await dependencies.hydrateGraph(graphPath, locale);
}

export async function executeEditorMutation(
  input: ExecuteEditorMutationInput,
  overrides: Partial<EditorMutationCoordinatorDependencies> = {},
): Promise<ExecuteEditorMutationOutcome> {
  const dependencies = { ...defaultDependencies, ...overrides };
  const identity = captureProjectIdentity();
  const basis = getGraphProjectionBasis(useGraphDataStore.getState(), input.graphPath);
  assertCurrentProjectIdentity(identity);
  if (!basis) throw new Error(`graph projection '${input.graphPath}' is not loaded`);

  const operationId = dependencies.createOperationId();
  const pending: PendingMutationRecord = {
    operationId,
    graphPath: input.graphPath,
    baseRevision: basis.graphRevision,
  };
  const request: MutationRequestDto<EditorGraphMutationDto> = {
    resource: { kind: 'graph', key: input.graphPath },
    baseRevision: pending.baseRevision,
    operationId,
    payload: input.mutation,
  };
  registerPendingMutation(pending);

  try {
    let result: GraphMutationResultDto;
    try {
      result = await dependencies.mutateGraph(
        identity.projectInstanceId,
        input.graphPath,
        input.locale,
        request,
      );
    } catch (error) {
      if (!isCurrentProjectIdentity(identity)
        || hasErrorCode(error, 'stale_project_lifecycle')) return { status: 'stale' };
      if (isRevisionConflict(error)) {
        await requestAuthoritativeHydrate(input.graphPath, input.locale, dependencies);
        return { status: 'conflict' };
      }
      const code = graphMutationErrorCode(error);
      if (code && code !== 'graph_revision_conflict') return { status: 'rejected', code };
      throw error;
    }

    if (!isCurrentProjectIdentity(identity)) return { status: 'stale', result };

    try {
      validateMutationResult(identity.projectInstanceId, pending, result);
      if (result.delta.toRevision === result.delta.fromRevision) {
        return { status: 'noop', result };
      }
      const applied = applyMutationResult(identity.projectInstanceId, pending, result);
      if (!applied.applied) {
        await requestAuthoritativeHydrate(input.graphPath, input.locale, dependencies);
        return { status: 'stale', result };
      }
    } catch (error) {
      await requestAuthoritativeHydrate(input.graphPath, input.locale, dependencies);
      throw error;
    }

    dependencies.updateHistoryStatus(result.history);
    return { status: 'applied', result };
  } finally {
    completePendingMutation(operationId);
  }
}



export function resetEditorMutationCoordinator(): void {
  resetPendingMutations();
}
