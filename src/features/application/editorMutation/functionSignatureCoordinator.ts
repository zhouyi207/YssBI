import { currentProjectionLocale, hydrateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';

import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { captureRevisionedProjectCommandSnapshot } from '@/features/application/projectCommandContext';

import { setHistoryStatus } from '@/features/application/editorMutation/historyCoordinator';
import {
  ProjectPublicationError,
  projectPublicationCoordinator,
} from './projectPublicationCoordinator';
import { hydrateFunctionSignaturesFromProjectIndex } from '@/features/application/graphDocument/functionSignatureSync';
import { dataTypeDisplay } from '@/shared/types/domain/dataType';
import type { FunctionSignaturePatch } from '@/shared/types';
import type {
  FunctionDocumentPatchDto,
  FunctionSignatureDto,
  HistoryStatusDto,
  MutationRequestDto,
  ResourceMutationResultDto,
} from '@/shared/types/domain/editorMutation';
import { FunctionMutationService } from '@/services/nodeSystem/functionMutationService';
import { isApplicationIpcErrorCode } from '@/features/application/errorReference';
import {
  ProjectService,
} from '@/services/project/projectService';
import type { ProjectGraphIndexRow } from '@/shared/types/domain/project';
import {
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  completePendingMutation,
  invalidatePendingMutation,
  registerPendingMutation,
  type PendingMutationRecord,
} from './pendingMutationRegistry';

export interface ExecuteFunctionSignatureMutationInput {
  functionPath: string;
  locale: string;
  patch: FunctionSignaturePatch;
}

export interface FunctionSignatureCoordinatorDependencies {
  createOperationId(): string;
  mutateSignature(
    projectInstanceId: string,
    functionPath: string,
    locale: string,
    request: MutationRequestDto<FunctionDocumentPatchDto>,
  ): Promise<ResourceMutationResultDto>;
  hydrateGraph(graphPath: string, locale: string): Promise<unknown>;
  loadFunctionResources(projectInstanceId: string): Promise<ProjectGraphIndexRow[]>;
  updateHistoryStatus(status: HistoryStatusDto): void;
}

export type ExecuteFunctionSignatureMutationOutcome =
  | { status: 'applied'; result: ResourceMutationResultDto }
  | { status: 'stale'; result?: ResourceMutationResultDto }
  | { status: 'conflict' };

let coordinatorEpoch = 0;
const pendingSignatureOperations = new Set<string>();

const defaultDependencies: FunctionSignatureCoordinatorDependencies = {
  createOperationId: () => crypto.randomUUID(),
  mutateSignature: (projectInstanceId, functionPath, locale, request) =>
    FunctionMutationService.updateSignature(projectInstanceId, functionPath, locale, request),
  hydrateGraph: hydrateGraphProjection,
  loadFunctionResources: async (projectInstanceId) => (
    await ProjectService.getProjectIndex(projectInstanceId)
  ).graphs,
  updateHistoryStatus: setHistoryStatus,
};



function isFunctionRevisionConflict(error: unknown): boolean {
  return isApplicationIpcErrorCode(error, 'function_revision_conflict');
}

function buildSignature(
  before: FunctionSignatureDto,
  patch: FunctionSignaturePatch,
): FunctionSignatureDto {
  const parameters = patch.inputs === undefined
    ? before.parameters
    : patch.inputs
        .filter((pin) => pin.dataType != null)
        .map((pin) => ({
          id: pin.id,
          name: pin.name,
          type_name: dataTypeDisplay(pin.dataType!),
        }));
  const returnType = patch.outputs === undefined
    ? before.return_type
    : patch.outputs.find((pin) => pin.dataType != null)?.dataType;
  return {
    parameters,
    return_type: typeof returnType === 'string'
      ? returnType
      : returnType
        ? dataTypeDisplay(returnType)
        : null,
  };
}

function validateDirectSignatureResult(
  pending: PendingMutationRecord,
  requestPatch: FunctionDocumentPatchDto,
  result: ResourceMutationResultDto,
): string | undefined {
  if (result.deltas.some((delta) => delta.causedBy !== pending.operationId)) {
    return 'operation correlation does not match the pending request';
  }
  const signatureDelta = result.deltas.find((delta) =>
    delta.resource.kind === 'function' && delta.resource.key === pending.graphPath,
  );
  if (!signatureDelta) return 'function signature delta is missing';
  if (signatureDelta.fromRevision !== pending.baseRevision) {
    return 'function signature revision does not match the request';
  }
  if (signatureDelta.payload.kind !== 'function'
    || JSON.stringify(signatureDelta.payload.patch) !== JSON.stringify(requestPatch)) {
    return 'function signature delta does not match the request payload';
  }
  return undefined;
}

async function hydrateAuthoritativeState(
  graphPaths: Iterable<string>,
  locale: string,
  dependencies: FunctionSignatureCoordinatorDependencies,
  identity: ProjectIdentitySnapshot,
): Promise<void> {
  const resources = await dependencies.loadFunctionResources(identity.projectInstanceId);
  if (!isCurrentProjectIdentity(identity)) return;
  hydrateFunctionSignaturesFromProjectIndex(resources);
  await Promise.all(
    [...new Set(graphPaths)].map((graphPath) => dependencies.hydrateGraph(graphPath, locale)),
  );
  if (!isCurrentProjectIdentity(identity)) return;
}



export async function executeFunctionSignatureMutation(
  input: ExecuteFunctionSignatureMutationInput,
  overrides: Partial<FunctionSignatureCoordinatorDependencies> = {},
): Promise<ExecuteFunctionSignatureMutationOutcome> {
  const dependencies = { ...defaultDependencies, ...overrides };
  const { context, authority: meta } = captureRevisionedProjectCommandSnapshot(
    () => useGraphMetaStore.getState().graphs[input.functionPath],
  );
  if (meta?.type !== 'function' || meta.functionRevision == null || !meta.functionSignature) {
    throw new Error(`function signature resource '${input.functionPath}' is not hydrated`);
  }

  const operationId = dependencies.createOperationId();
  const requestPatch: FunctionDocumentPatchDto = {
    before: meta.functionSignature,
    after: buildSignature(meta.functionSignature, input.patch),
  };
  const pending: PendingMutationRecord = {
    operationId,
    graphPath: input.functionPath,
    baseRevision: meta.functionRevision,
  };
  const request: MutationRequestDto<FunctionDocumentPatchDto> = {
    resource: { kind: 'function', key: input.functionPath },
    baseRevision: meta.functionRevision,
    operationId,
    payload: requestPatch,
  };
  const epoch = coordinatorEpoch;
  const identity: ProjectIdentitySnapshot = {
    projectInstanceId: context.projectInstanceId,
    epoch: context.projectEpoch,
  };
  registerPendingMutation(pending);
  pendingSignatureOperations.add(operationId);

  try {
    let result: ResourceMutationResultDto;
    try {
      result = await dependencies.mutateSignature(
        identity.projectInstanceId,
        input.functionPath,
        input.locale,
        request,
      );
      if (!isCurrentProjectIdentity(identity)) return { status: 'stale', result };
    } catch (error) {
      if (!isCurrentProjectIdentity(identity)
        || isApplicationIpcErrorCode(error, 'stale_project_lifecycle')) return { status: 'stale' };
      if (epoch !== coordinatorEpoch) return { status: 'stale' };
      if (!isFunctionRevisionConflict(error)) throw error;
      await hydrateAuthoritativeState(
        [input.functionPath],
        input.locale,
        dependencies,
        identity,
      );
      if (!isCurrentProjectIdentity(identity) || epoch !== coordinatorEpoch) {
        return { status: 'stale' };
      }
      return { status: 'conflict' };
    }

    if (epoch !== coordinatorEpoch) return { status: 'stale', result };
    try {
      await projectPublicationCoordinator.submit({
        result,
        fallbackPaths: [input.functionPath],
        validate: (candidate) => validateDirectSignatureResult(pending, requestPatch, candidate),
      });
    } catch (error) {
      if (error instanceof ProjectPublicationError && error.code === 'stale_project_lifecycle') {
        return { status: 'stale', result };
      }
      throw error;
    }
    if (epoch !== coordinatorEpoch) return { status: 'stale', result };
    return { status: 'applied', result };
  } finally {
    completePendingMutation(operationId);
    pendingSignatureOperations.delete(operationId);
  }
}

export function commitFunctionSignature(
  functionPath: string,
  patch: FunctionSignaturePatch,
): Promise<ExecuteFunctionSignatureMutationOutcome> {
  return executeFunctionSignatureMutation({
    functionPath,
    locale: currentProjectionLocale(),
    patch,
  });
}

export function resetFunctionSignatureCoordinator(): void {
  coordinatorEpoch += 1;
  for (const operationId of pendingSignatureOperations) invalidatePendingMutation(operationId);
  pendingSignatureOperations.clear();
}
