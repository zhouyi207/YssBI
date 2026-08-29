import type { ErrorReference } from '@/features/application/errorReference';
import type {
  GraphDeltaDto,
  ResourceKeyDto,
  ResourceMutationResultDto,
} from '@/shared/types/domain/editorMutation';
import type {
  ComputationSettingsMutationReceiptDto,
} from '@/shared/types/domain/projectComputationSettings';
import type {
  LifecycleMutationResultDto,
} from '@/shared/types/domain/project';
import type {
  ProjectHydrationCoordinator,
  ProjectHydrationOutcome,
} from './projectHydrationCoordinator';

type Awaitable<T> = T | PromiseLike<T>;

export interface ProjectSaveReceipt {
  readonly projectInstanceId: string;
  readonly operationId: string;
  readonly publicationRevision: number;
  readonly affectedResources: readonly ResourceKeyDto[];
  readonly indexInvalidated: boolean;
  readonly history: {
    readonly canUndo: boolean;
    readonly canRedo: boolean;
  };
}

export interface ProjectLoadedPayload {
  readonly result: {
    readonly path: string;
    readonly projectInstanceId: string;
    readonly activationRevision: number;
  };
}

export interface ProjectLifecycleCommittedPayload {
  readonly result: LifecycleMutationResultDto;
}

export interface ProjectSavedPayload {
  readonly result: ProjectSaveReceipt;
}

export interface ComputationSettingsChangedPayload {
  readonly result: ComputationSettingsMutationReceiptDto;
}

export interface ProjectIndexInvalidatedPayload {
  readonly projectInstanceId: string;
  readonly source: 'watcher';
  readonly version: number;
}

export interface GraphDeltaEventPayload {
  readonly projectInstanceId: string;
  readonly delta: GraphDeltaDto;
}

export interface ResourceMutationCommittedPayload {
  readonly result: ResourceMutationResultDto;
}

export type ProjectEvent =
  | { readonly type: 'ProjectLoaded'; readonly payload: ProjectLoadedPayload }
  | { readonly type: 'ProjectCleared'; readonly payload: undefined }
  | {
      readonly type: 'ProjectLifecycleCommitted';
      readonly payload: ProjectLifecycleCommittedPayload;
    }
  | { readonly type: 'ProjectSaved'; readonly payload: ProjectSavedPayload }
  | {
      readonly type: 'ComputationSettingsChanged';
      readonly payload: ComputationSettingsChangedPayload;
    }
  | {
      readonly type: 'ProjectIndexInvalidated';
      readonly payload: ProjectIndexInvalidatedPayload;
    }
  | { readonly type: 'GraphDelta'; readonly payload: GraphDeltaEventPayload }
  | {
      readonly type: 'ResourceMutationCommitted';
      readonly payload: ResourceMutationCommittedPayload;
    };

export interface OptimisticOperationKey {
  readonly projectInstanceId: string;
  readonly resourceKey: string;
  readonly operationId: string;
  readonly fromRevision: number;
}

export type ProjectReconciliationOutcome =
  | { readonly status: 'applied' }
  | { readonly status: 'duplicate' }
  | { readonly status: 'ignored' }
  | { readonly status: 'recoveryRequested' };

export type ProjectRecoveryReason =
  | 'unknownOutcome'
  | 'reconcilerRejected'
  | 'hydrationFailed';

export interface ProjectEventReconcilerDependencies {
  readonly hydration: Pick<
    ProjectHydrationCoordinator,
    'loadCurrentProject' | 'refreshResourceIndex' | 'loadGraph' | 'replaceProject'
  >;
  readonly activateProject?: (
    result: ProjectLoadedPayload['result'],
  ) => Awaitable<boolean>;
  readonly currentProjectInstanceId: () => string | null;
  readonly publishProjectCleared?: () => Awaitable<void>;
  readonly publishLifecycleCommitted?: (
    result: LifecycleMutationResultDto,
  ) => Awaitable<void>;
  readonly publishProjectSaved?: (result: ProjectSaveReceipt) => Awaitable<void>;
  readonly publishComputationSettingsChanged?: (
    result: ComputationSettingsMutationReceiptDto,
  ) => Awaitable<void>;
  readonly publishGraphDelta?: (payload: GraphDeltaEventPayload) => Awaitable<void>;
  readonly publishResourceMutationCommitted?: (
    result: ResourceMutationResultDto,
  ) => Awaitable<void>;
  readonly settleOptimisticOperation?: (key: OptimisticOperationKey) => void;
  readonly rejectOptimisticOperation?: (
    key: OptimisticOperationKey,
    failure: ErrorReference,
  ) => void;
  readonly invalidateOptimisticOperation?: (key: OptimisticOperationKey) => void;
  readonly requestAuthoritativeSnapshot?: (
    reason: ProjectRecoveryReason,
  ) => Awaitable<void>;
}

export interface ProjectEventReconciler {
  acceptEvent(event: ProjectEvent): Promise<ProjectReconciliationOutcome>;
  acceptCommittedReceipt(result: ResourceMutationResultDto): Promise<ProjectReconciliationOutcome>;
  acknowledgeOperation(key: OptimisticOperationKey): void;
  rejectOperation(key: OptimisticOperationKey, failure: ErrorReference): void;
  markUnknownOutcome(key: OptimisticOperationKey): Promise<ProjectReconciliationOutcome>;
  resetForProject(projectInstanceId: string | null): void;
}

interface OperationRecord {
  readonly key: OptimisticOperationKey;
  pending: boolean;
  committed: boolean;
  settled: boolean;
}

const MAX_OPERATION_RECORDS = 256;

function operationRecordId(key: OptimisticOperationKey): string {
  return [
    key.projectInstanceId,
    key.resourceKey,
    key.operationId,
    key.fromRevision,
  ].join('\u001f');
}

function resourceKey(resource: ResourceKeyDto): string {
  return resource.kind === 'graph' ? resource.key : `${resource.kind}:${resource.key}`;
}

function stableValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stableValue);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, nested]) => [key, stableValue(nested)]),
  );
}

function fingerprint(value: unknown): string {
  return JSON.stringify(stableValue(value));
}

function hydrationOutcomeToReconciliation(
  outcome: ProjectHydrationOutcome,
): ProjectReconciliationOutcome {
  if (outcome.status === 'published') return { status: 'applied' };
  if (outcome.status === 'failed') return { status: 'recoveryRequested' };
  return { status: 'ignored' };
}

export function createProjectEventReconciler(
  dependencies: ProjectEventReconcilerDependencies,
): ProjectEventReconciler {
  const operationRecords = new Map<string, OperationRecord>();
  const seenEvents = new Map<string, string>();
  const seenReceipts = new Set<string>();
  const recoveryRequests = new Set<string>();

  const rememberOperation = (key: OptimisticOperationKey): OperationRecord => {
    const id = operationRecordId(key);
    const existing = operationRecords.get(id);
    if (existing) return existing;
    if (operationRecords.size >= MAX_OPERATION_RECORDS) {
      const oldest = operationRecords.keys().next().value;
      if (oldest !== undefined) operationRecords.delete(oldest);
    }
    const created: OperationRecord = {
      key,
      pending: false,
      committed: false,
      settled: false,
    };
    operationRecords.set(id, created);
    return created;
  };

  const settle = (record: OperationRecord): void => {
    if (!record.pending || record.settled) return;
    record.pending = false;
    record.settled = true;
    try {
      dependencies.settleOptimisticOperation?.(record.key);
    } catch {
      // Overlay bookkeeping must not change the authoritative event outcome.
    }
  };

  const requestRecovery = async (
    reason: ProjectRecoveryReason,
    requestId: string,
  ): Promise<ProjectReconciliationOutcome> => {
    if (recoveryRequests.has(requestId)) return { status: 'recoveryRequested' };
    recoveryRequests.add(requestId);
    try {
      await dependencies.requestAuthoritativeSnapshot?.(reason);
    } catch {
      // Recovery remains the safe result even if the recovery request is rejected.
    }
    return { status: 'recoveryRequested' };
  };

  const acceptGraphDelta = async (
    payload: GraphDeltaEventPayload,
  ): Promise<ProjectReconciliationOutcome> => {
    if (dependencies.currentProjectInstanceId() !== payload.projectInstanceId) {
      return { status: 'ignored' };
    }
    const eventFingerprint = fingerprint(payload);
    const eventKey = payload.delta.causedBy
      ? operationRecordId({
          projectInstanceId: payload.projectInstanceId,
          resourceKey: payload.delta.graphPath,
          operationId: payload.delta.causedBy,
          fromRevision: payload.delta.fromRevision,
        })
      : `graph:${eventFingerprint}`;
    const previous = seenEvents.get(eventKey);
    if (previous === eventFingerprint) return { status: 'duplicate' };
    if (previous !== undefined) {
      return { status: 'recoveryRequested' };
    }

    const operation = payload.delta.causedBy
      ? rememberOperation({
          projectInstanceId: payload.projectInstanceId,
          resourceKey: payload.delta.graphPath,
          operationId: payload.delta.causedBy,
          fromRevision: payload.delta.fromRevision,
        })
      : undefined;
    if (operation?.committed) {
      seenEvents.set(eventKey, eventFingerprint);
      return { status: 'duplicate' };
    }
    await dependencies.publishGraphDelta?.(payload);
    seenEvents.set(eventKey, eventFingerprint);
    if (operation) {
      operation.committed = true;
      settle(operation);
    }
    return { status: 'applied' };
  };

  const acceptResourceReceipt = async (
    result: ResourceMutationResultDto,
  ): Promise<ProjectReconciliationOutcome> => {
    if (dependencies.currentProjectInstanceId() !== result.projectInstanceId) {
      return { status: 'ignored' };
    }
    const resultFingerprint = fingerprint(result);
    if (seenReceipts.has(resultFingerprint)) return { status: 'duplicate' };
    const operations = result.deltas
      .filter((delta) => delta.causedBy)
      .map((delta) => rememberOperation({
        projectInstanceId: result.projectInstanceId,
        resourceKey: resourceKey(delta.resource),
        operationId: delta.causedBy!,
        fromRevision: delta.fromRevision,
      }));
    if (operations.length > 0 && operations.every((operation) => operation.committed)) {
      seenReceipts.add(resultFingerprint);
      return { status: 'duplicate' };
    }
    await dependencies.publishResourceMutationCommitted?.(result);
    seenReceipts.add(resultFingerprint);
    for (const operation of operations) {
      operation.committed = true;
      settle(operation);
    }
    return { status: 'applied' };
  };

  const acceptEventInternal = async (
    event: ProjectEvent,
  ): Promise<ProjectReconciliationOutcome> => {
    switch (event.type) {
      case 'ProjectLoaded': {
        if (dependencies.activateProject) {
          const activated = await dependencies.activateProject(event.payload.result);
          if (!activated) return { status: 'ignored' };
          resetForProject(event.payload.result.projectInstanceId);
          return { status: 'applied' };
        }
        dependencies.hydration.replaceProject();
        resetForProject(event.payload.result.projectInstanceId);
        return hydrationOutcomeToReconciliation(
          await dependencies.hydration.loadCurrentProject(),
        );
      }
      case 'ProjectCleared':
        dependencies.hydration.replaceProject();
        resetForProject(null);
        await dependencies.publishProjectCleared?.();
        return { status: 'applied' };
      case 'ProjectLifecycleCommitted':
        await dependencies.publishLifecycleCommitted?.(event.payload.result);
        return { status: 'applied' };
      case 'ProjectSaved':
        if (
          dependencies.currentProjectInstanceId() !== event.payload.result.projectInstanceId
        ) {
          return { status: 'ignored' };
        }
        await dependencies.publishProjectSaved?.(event.payload.result);
        return { status: 'applied' };
      case 'ComputationSettingsChanged':
        if (
          dependencies.currentProjectInstanceId()
          !== event.payload.result.projectInstanceId
        ) {
          return { status: 'ignored' };
        }
        await dependencies.publishComputationSettingsChanged?.(event.payload.result);
        return { status: 'applied' };
      case 'ProjectIndexInvalidated':
        if (
          dependencies.currentProjectInstanceId() !== event.payload.projectInstanceId
        ) {
          return { status: 'ignored' };
        }
        return hydrationOutcomeToReconciliation(
          await dependencies.hydration.refreshResourceIndex(),
        );
      case 'GraphDelta':
        return acceptGraphDelta(event.payload);
      case 'ResourceMutationCommitted':
        return acceptResourceReceipt(event.payload.result);
    }
  };

  const acceptEvent = async (
    event: ProjectEvent,
  ): Promise<ProjectReconciliationOutcome> => {
    try {
      return await acceptEventInternal(event);
    } catch {
      return { status: 'recoveryRequested' };
    }
  };

  const acceptCommittedReceipt = async (
    result: ResourceMutationResultDto,
  ): Promise<ProjectReconciliationOutcome> => {
    try {
      return await acceptResourceReceipt(result);
    } catch {
      return { status: 'recoveryRequested' };
    }
  };

  const acknowledgeOperation = (key: OptimisticOperationKey): void => {
    if (dependencies.currentProjectInstanceId() !== key.projectInstanceId) return;
    const record = rememberOperation(key);
    if (record.settled && !record.committed) return;
    if (record.committed) {
      record.pending = true;
      settle(record);
      return;
    }
    record.pending = true;
  };

  const rejectOperation = (
    key: OptimisticOperationKey,
    failure: ErrorReference,
  ): void => {
    const record = operationRecords.get(operationRecordId(key));
    if (!record || !record.pending || record.committed) return;
    record.pending = false;
    record.settled = true;
    try {
      dependencies.rejectOptimisticOperation?.(key, failure);
    } catch {
      // A rejected overlay is already isolated from authoritative state.
    }
  };

  const markUnknownOutcome = async (
    key: OptimisticOperationKey,
  ): Promise<ProjectReconciliationOutcome> => {
    if (dependencies.currentProjectInstanceId() !== key.projectInstanceId) {
      return { status: 'ignored' };
    }
    const record = operationRecords.get(operationRecordId(key));
    if (record?.pending && !record.committed) {
      record.pending = false;
      record.settled = true;
      try {
        dependencies.invalidateOptimisticOperation?.(key);
      } catch {
        // Invalidating the overlay is best effort; authoritative recovery is mandatory.
      }
    }
    return requestRecovery('unknownOutcome', operationRecordId(key));
  };

  function resetForProject(projectInstanceId: string | null): void {
    for (const [id, record] of operationRecords) {
      if (projectInstanceId === null || record.key.projectInstanceId !== projectInstanceId) {
        operationRecords.delete(id);
      }
    }
    seenEvents.clear();
    seenReceipts.clear();
    recoveryRequests.clear();
  }

  return {
    acceptEvent,
    acceptCommittedReceipt,
    acknowledgeOperation,
    rejectOperation,
    markUnknownOutcome,
    resetForProject,
  };
}
