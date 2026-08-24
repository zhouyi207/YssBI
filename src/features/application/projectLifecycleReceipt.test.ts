import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { ProjectLifecycleCommittedHandler } from '@/features/core/sync/handlers/ProjectEventHandler';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import type { LifecycleMutationResultDto } from '@/shared/types/dto/project';
import { logger } from '@/utils/appLogger';
import {
  installCoreApplicationTestPorts,
  resetCoreApplicationTestPorts,
} from '@/features/application/testHelpers/coreApplicationPorts';
import {
  ProjectLifecycleProtocolError,
  PROJECT_LIFECYCLE_SETTLEMENT_TTL_MS,
  applyProjectLifecycleReceipt,
  claimProjectLifecycleNotification,
  getProjectLifecycleRegistrySizeForTests,
  recoverProjectLifecycleDirectFailure,
  registerPendingProjectLifecycleOperation,
  resetProjectLifecycleReceiptHandlerForTests,
  setProjectLifecycleClockForTests,
  type ProjectLifecycleReceiptDependencies,
} from './projectLifecycleReceipt';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function startProject(projectInstanceId: string, revision = 0): void {
  useProjectIOStore.setState({ projectInstanceId });
  projectPublicationCoordinator.startProject(projectInstanceId, revision);
}

function receipt(
  operationId: string,
  patch: Partial<LifecycleMutationResultDto> = {},
): LifecycleMutationResultDto {
  return {
    operationId,
    kind: 'saveAs',
    oldProjectInstanceId: 'project-a',
    newProjectInstanceId: 'project-b',
    phase: 'authorityCommitted',
    outcome: 'committed',
    record: null,
    path: 'C:/project-b/metadata.yssbi',
    recovery: null,
    invalidation: { project: true, registry: true },
    ...patch,
  };
}

function dependencies(
  patch: Partial<ProjectLifecycleReceiptDependencies> = {},
): ProjectLifecycleReceiptDependencies {
  return {
    prepareProjectTransition: vi.fn(async () => ({
      projectInstanceId: 'project-b',
      publicationRevision: 0,
      commit: async () => startProject('project-b', 0),
    })),
    refreshRegistry: vi.fn(async () => []),
    clearProject: vi.fn(async () => {
      useProjectIOStore.setState({ projectInstanceId: null });
    }),
    markProjectStale: vi.fn(),
    ...patch,
  };
}

describe('project lifecycle pending receipt registry', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setProjectLifecycleClockForTests(() => 1_000);
    vi.spyOn(logger.sys, 'error').mockImplementation(() => undefined);
    resetProjectLifecycleReceiptHandlerForTests();
    installCoreApplicationTestPorts({
      syncEvents: {
        applyProjectLifecycleReceipt: async (result, deps) => {
          await applyProjectLifecycleReceipt(
            result as LifecycleMutationResultDto,
            'event',
            deps as ProjectLifecycleReceiptDependencies,
          );
        },
      },
    });
    startProject('project-a', 4);
  });

  afterEach(resetCoreApplicationTestPorts);

  it.each(['event-first', 'direct-first'] as const)(
    '%s matching DTOs perform shared effects once while direct keeps notification ownership',
    async (order) => {
      const pending = registerPendingProjectLifecycleOperation({ kind: 'saveAs' });
      const result = receipt(pending.operationId);
      const deps = dependencies();
      const handler = new ProjectLifecycleCommittedHandler(deps);

      if (order === 'event-first') {
        handler.handle({ result: structuredClone(result) });
        await vi.waitFor(() => expect(deps.refreshRegistry).toHaveBeenCalledOnce());
        await applyProjectLifecycleReceipt(result, 'direct', deps);
      } else {
        await applyProjectLifecycleReceipt(result, 'direct', deps);
        handler.handle({ result: structuredClone(result) });
        await Promise.resolve();
      }

      expect(deps.prepareProjectTransition).toHaveBeenCalledOnce();
      expect(deps.refreshRegistry).toHaveBeenCalledOnce();
      expect(claimProjectLifecycleNotification(pending.operationId)).toBe(true);
      expect(claimProjectLifecycleNotification(pending.operationId)).toBe(false);
    },
  );

  it('keeps event-first active delete ownership while direct delivery joins the deferred clear', async () => {
    const pending = registerPendingProjectLifecycleOperation({ kind: 'delete' });
    const result = receipt(pending.operationId, {
      kind: 'delete',
      newProjectInstanceId: null,
    });
    const clear = deferred<void>();
    let clearOwner: unknown;
    const deps = dependencies({
      clearProject: vi.fn(async (...args: unknown[]) => {
        [clearOwner] = args;
        await clear.promise;
        useProjectIOStore.setState({ projectInstanceId: null });
      }),
    });

    new ProjectLifecycleCommittedHandler(deps).handle({ result: structuredClone(result) });
    await vi.waitFor(() => expect(deps.clearProject).toHaveBeenCalledOnce());
    const ownerWhileDeferred = projectPublicationCoordinator.getSnapshotForTests();
    const remainedCurrentWhileDeferred = pending.isCurrent();
    const direct = applyProjectLifecycleReceipt(result, 'direct', deps);
    await Promise.resolve();
    clear.resolve();

    await expect(direct).resolves.toMatchObject({ status: 'duplicate' });
    expect(remainedCurrentWhileDeferred).toBe(true);
    expect(ownerWhileDeferred.projectInstanceId).toBeNull();
    expect(clearOwner).toEqual({
      projectInstanceId: null,
      epoch: ownerWhileDeferred.epoch,
    });
    expect(Object.isFrozen(clearOwner)).toBe(true);
    expect(deps.clearProject).toHaveBeenCalledOnce();
    expect(deps.refreshRegistry).toHaveBeenCalledOnce();
    expect(claimProjectLifecycleNotification(pending.operationId)).toBe(true);
    expect(claimProjectLifecycleNotification(pending.operationId)).toBe(false);
  });

  it('allows direct delivery to retry when event-first processing fails', async () => {
    const pending = registerPendingProjectLifecycleOperation({ kind: 'saveAs' });
    const result = receipt(pending.operationId);
    const failed = dependencies({
      prepareProjectTransition: vi.fn(async () => { throw new Error('event hydrate failed'); }),
    });
    const recovered = dependencies();

    new ProjectLifecycleCommittedHandler(failed).handle({ result });
    await vi.waitFor(() => expect(failed.prepareProjectTransition).toHaveBeenCalledOnce());
    await expect(applyProjectLifecycleReceipt(result, 'direct', recovered)).resolves.toMatchObject({
      status: 'applied',
    });

    expect(recovered.prepareProjectTransition).toHaveBeenCalledOnce();
    expect(recovered.refreshRegistry).toHaveBeenCalledOnce();
  });

  it('rejects a mismatching direct DTO as a protocol error with zero direct effects', async () => {
    const pending = registerPendingProjectLifecycleOperation({ kind: 'saveAs' });
    const hydration = deferred<{
      projectInstanceId: string;
      publicationRevision: number;
      commit(): Promise<void>;
    } | null>();
    const eventDeps = dependencies({ prepareProjectTransition: vi.fn(() => hydration.promise) });
    const directDeps = dependencies();
    const eventResult = receipt(pending.operationId);

    new ProjectLifecycleCommittedHandler(eventDeps).handle({ result: eventResult });
    await vi.waitFor(() => expect(eventDeps.prepareProjectTransition).toHaveBeenCalledOnce());
    await expect(applyProjectLifecycleReceipt(
      receipt(pending.operationId, { path: 'C:/different/metadata.yssbi' }),
      'direct',
      directDeps,
    )).rejects.toBeInstanceOf(ProjectLifecycleProtocolError);

    expect(directDeps.prepareProjectTransition).not.toHaveBeenCalled();
    expect(directDeps.refreshRegistry).not.toHaveBeenCalled();
    expect(directDeps.clearProject).not.toHaveBeenCalled();
    expect(directDeps.markProjectStale).not.toHaveBeenCalled();
    hydration.resolve({
      projectInstanceId: 'project-b',
      publicationRevision: 0,
      commit: async () => startProject('project-b', 0),
    });
    await vi.waitFor(() => expect(eventDeps.refreshRegistry).toHaveBeenCalledOnce());
  });

  it('gives an external event without a pending operation zero effects', async () => {
    const deps = dependencies();

    new ProjectLifecycleCommittedHandler(deps).handle({
      result: receipt('external-operation'),
    });
    await Promise.resolve();
    await Promise.resolve();

    expect(deps.prepareProjectTransition).not.toHaveBeenCalled();
    expect(deps.refreshRegistry).not.toHaveBeenCalled();
    expect(deps.clearProject).not.toHaveBeenCalled();
    expect(deps.markProjectStale).not.toHaveBeenCalled();
  });

  it('rejects a late event from an earlier epoch of the same project instance', async () => {
    const pending = registerPendingProjectLifecycleOperation({ kind: 'saveAs' });
    const deps = dependencies();
    startProject('project-a', 9);

    new ProjectLifecycleCommittedHandler(deps).handle({
      result: receipt(pending.operationId),
    });
    await Promise.resolve();
    await Promise.resolve();

    expect(deps.prepareProjectTransition).not.toHaveBeenCalled();
    expect(deps.refreshRegistry).not.toHaveBeenCalled();
  });

  it('uses the coordinator application generation for no-project create and inactive delete', async () => {
    projectPublicationCoordinator.cancelProject();
    useProjectIOStore.setState({ projectInstanceId: null });
    const create = registerPendingProjectLifecycleOperation({ kind: 'create' });
    const inactiveDelete = registerPendingProjectLifecycleOperation({ kind: 'delete' });
    const deps = dependencies();
    projectPublicationCoordinator.cancelProject();

    await expect(applyProjectLifecycleReceipt(receipt(create.operationId, {
      kind: 'create',
      oldProjectInstanceId: null,
      newProjectInstanceId: null,
      phase: 'registryCommitted',
      invalidation: { project: false, registry: true },
    }), 'direct', deps)).resolves.toMatchObject({ status: 'stale' });
    await expect(applyProjectLifecycleReceipt(receipt(inactiveDelete.operationId, {
      kind: 'delete',
      oldProjectInstanceId: null,
      newProjectInstanceId: null,
      invalidation: { project: false, registry: true },
    }), 'direct', deps)).resolves.toMatchObject({ status: 'stale' });

    expect(deps.refreshRegistry).not.toHaveBeenCalled();
  });

  it.each([
    ['registryFailed', false],
    ['activationFailed', true],
  ] as const)(
    '%s performs its explicit refresh and rehydrate recovery policy',
    async (outcome, shouldRehydrate) => {
      const kind = 'saveAs' as const;
      const pending = registerPendingProjectLifecycleOperation({ kind });
      const deps = dependencies();
      const result = receipt(pending.operationId, {
        kind,
        outcome,
        newProjectInstanceId: null,
        phase: outcome === 'registryFailed' ? 'destinationCommitted' : 'registryCommitted',
        recovery: { required: true, action: outcome, path: 'C:/recovery', identity: null },
        invalidation: { project: outcome === 'activationFailed', registry: true },
      });

      await applyProjectLifecycleReceipt(result, 'direct', deps);

      expect(deps.refreshRegistry).toHaveBeenCalledOnce();
      expect(deps.prepareProjectTransition).toHaveBeenCalledTimes(shouldRehydrate ? 1 : 0);
      expect(deps.markProjectStale).not.toHaveBeenCalled();
    },
  );

  it.each(['event-first', 'direct-first'] as const)(
    '%s settles an active terminal-row rejection with one registry refresh and no project effects',
    async (order) => {
      const pending = registerPendingProjectLifecycleOperation({
        kind: 'delete',
        expectsActiveProject: true,
      });
      const result = {
        ...receipt(pending.operationId),
        kind: 'registryCleanup',
        oldProjectInstanceId: null,
        newProjectInstanceId: null,
        phase: 'registryCommitted',
        outcome: 'registryFailed',
        record: {
          id: 'invalid-active-row',
          name: 'Invalid active row',
          path: 'C:/project-a/metadata.yssbi',
          createdAt: '2026-07-29T00:00:00Z',
          lastOpenedAt: null,
          isFavorite: false,
          rootIdentity: '',
        },
        path: null,
        recovery: {
          required: true,
          action: 'cleanupRegistry',
          path: null,
          identity: null,
        },
        invalidation: { project: false, registry: true },
      } as LifecycleMutationResultDto;
      const deps = dependencies();
      const handler = new ProjectLifecycleCommittedHandler(deps);

      if (order === 'event-first') {
        handler.handle({ result: structuredClone(result) });
        await vi.waitFor(() => expect(deps.refreshRegistry).toHaveBeenCalledOnce());
        await expect(applyProjectLifecycleReceipt(result, 'direct', deps)).resolves.toMatchObject({
          status: 'duplicate',
        });
      } else {
        await expect(applyProjectLifecycleReceipt(result, 'direct', deps)).resolves.toMatchObject({
          status: 'applied',
        });
        handler.handle({ result: structuredClone(result) });
        await Promise.resolve();
      }

      expect(deps.refreshRegistry).toHaveBeenCalledOnce();
      expect(deps.clearProject).not.toHaveBeenCalled();
      expect(deps.prepareProjectTransition).not.toHaveBeenCalled();
      expect(deps.markProjectStale).not.toHaveBeenCalled();
      expect(useProjectIOStore.getState().projectInstanceId).toBe('project-a');
      expect(claimProjectLifecycleNotification(pending.operationId)).toBe(true);

      const late = dependencies();
      await expect(applyProjectLifecycleReceipt(result, 'event', late)).resolves.toMatchObject({
        status: 'stale',
      });
      expect(late.refreshRegistry).not.toHaveBeenCalled();
      expect(late.clearProject).not.toHaveBeenCalled();
      expect(late.prepareProjectTransition).not.toHaveBeenCalled();
      expect(late.markProjectStale).not.toHaveBeenCalled();
    },
  );

  it('accepts registry-only cleanup for an inactive delete request without clearing authority', async () => {
    const pending = registerPendingProjectLifecycleOperation({
      kind: 'delete',
      expectsActiveProject: false,
    });
    const deps = dependencies();
    const result = {
      ...receipt(pending.operationId),
      kind: 'registryCleanup',
      oldProjectInstanceId: null,
      newProjectInstanceId: null,
      phase: 'registryCommitted',
      outcome: 'committed',
      invalidation: { project: false, registry: true },
    } as LifecycleMutationResultDto;

    await expect(applyProjectLifecycleReceipt(result, 'direct', deps)).resolves.toMatchObject({
      status: 'applied',
    });

    expect(deps.clearProject).not.toHaveBeenCalled();
    expect(deps.prepareProjectTransition).not.toHaveBeenCalled();
    expect(deps.refreshRegistry).toHaveBeenCalledOnce();
  });

  it('clears active authority and refreshes registry for cleanup-pending delete', async () => {
    const pending = registerPendingProjectLifecycleOperation({ kind: 'delete' });
    const deps = dependencies();

    await applyProjectLifecycleReceipt(receipt(pending.operationId, {
      kind: 'delete',
      newProjectInstanceId: null,
      outcome: 'cleanupPending',
      recovery: {
        required: true,
        action: 'cleanupTombstone',
        path: 'C:/.project-a.yssbi-deleting-operation',
        identity: 'native-id',
      },
    }), 'direct', deps);

    expect(deps.clearProject).toHaveBeenCalledOnce();
    expect(deps.refreshRegistry).toHaveBeenCalledOnce();
    expect(projectPublicationCoordinator.getSnapshotForTests().projectInstanceId).toBeNull();
  });

  it('evicts completed receipts before rejecting a new pending operation', async () => {
    const completed = registerPendingProjectLifecycleOperation({
      kind: 'create',
      operationId: 'completed-operation',
    });
    await applyProjectLifecycleReceipt(receipt(completed.operationId, {
      kind: 'create',
      oldProjectInstanceId: null,
      newProjectInstanceId: null,
      phase: 'registryCommitted',
      invalidation: { project: false, registry: true },
    }), 'direct', dependencies());
    expect(claimProjectLifecycleNotification(completed.operationId)).toBe(true);
    for (let index = 0; index < 127; index += 1) {
      registerPendingProjectLifecycleOperation({
        kind: 'create',
        operationId: `pending-${index}`,
      });
    }

    expect(() => registerPendingProjectLifecycleOperation({
      kind: 'create',
      operationId: 'replacement-operation',
    })).not.toThrow();
    expect(getProjectLifecycleRegistrySizeForTests()).toBe(128);
  });

  it('does not evict an event-first completion before its direct notification is claimed', async () => {
    const unnotified = registerPendingProjectLifecycleOperation({
      kind: 'create',
      operationId: 'unnotified-operation',
    });
    await applyProjectLifecycleReceipt(receipt(unnotified.operationId, {
      kind: 'create',
      oldProjectInstanceId: null,
      newProjectInstanceId: null,
      phase: 'registryCommitted',
      invalidation: { project: false, registry: true },
    }), 'event', dependencies());
    for (let index = 0; index < 127; index += 1) {
      registerPendingProjectLifecycleOperation({
        kind: 'create',
        operationId: `protected-${index}`,
      });
    }

    expect(() => registerPendingProjectLifecycleOperation({
      kind: 'create',
      operationId: 'must-not-evict-notification',
    })).toThrow(ProjectLifecycleProtocolError);
    expect(claimProjectLifecycleNotification(unnotified.operationId)).toBe(true);
  });

  it('sweeps 128 stale generation entries and admits a new operation', () => {
    for (let index = 0; index < 128; index += 1) {
      registerPendingProjectLifecycleOperation({
        kind: 'saveAs',
        operationId: `stale-${index}`,
      });
    }
    startProject('project-replacement', 0);

    expect(() => registerPendingProjectLifecycleOperation({
      kind: 'saveAs',
      operationId: 'post-replacement',
    })).not.toThrow();
    expect(getProjectLifecycleRegistrySizeForTests()).toBe(1);
  });

  it('waits for an event processing attempt before settling direct failure', async () => {
    const pending = registerPendingProjectLifecycleOperation({ kind: 'saveAs' });
    const registry = deferred<[]>();
    const deps = dependencies({ refreshRegistry: vi.fn(() => registry.promise) });
    new ProjectLifecycleCommittedHandler(deps).handle({
      result: receipt(pending.operationId),
    });
    await vi.waitFor(() => expect(deps.refreshRegistry).toHaveBeenCalledOnce());

    const recovered = recoverProjectLifecycleDirectFailure(pending.operationId);
    registry.resolve([]);

    await expect(recovered).resolves.toMatchObject({
      status: 'applied',
      result: { operationId: pending.operationId },
    });
  });

  it('expires a transport-lost pending operation and admits a new operation', async () => {
    let now = 1_000;
    setProjectLifecycleClockForTests(() => now);
    const pending = registerPendingProjectLifecycleOperation({ kind: 'saveAs' });

    await expect(recoverProjectLifecycleDirectFailure(pending.operationId)).resolves.toBeUndefined();
    now += PROJECT_LIFECYCLE_SETTLEMENT_TTL_MS + 1;
    registerPendingProjectLifecycleOperation({ kind: 'saveAs', operationId: 'after-transport-ttl' });

    expect(getProjectLifecycleRegistrySizeForTests()).toBe(1);
  });

  it('treats the first late event after direct-loss expiry as stale with zero effects', async () => {
    let now = 1_000;
    setProjectLifecycleClockForTests(() => now);
    const pending = registerPendingProjectLifecycleOperation({ kind: 'saveAs' });
    const deps = dependencies();
    await recoverProjectLifecycleDirectFailure(pending.operationId);
    now += PROJECT_LIFECYCLE_SETTLEMENT_TTL_MS + 1;

    await expect(applyProjectLifecycleReceipt(
      receipt(pending.operationId),
      'event',
      deps,
    )).resolves.toMatchObject({ status: 'stale' });

    expect(deps.prepareProjectTransition).not.toHaveBeenCalled();
    expect(deps.refreshRegistry).not.toHaveBeenCalled();
    expect(deps.clearProject).not.toHaveBeenCalled();
    expect(deps.markProjectStale).not.toHaveBeenCalled();
  });

  it('expires event-only completion when no direct caller ever claims it', async () => {
    let now = 1_000;
    setProjectLifecycleClockForTests(() => now);
    const pending = registerPendingProjectLifecycleOperation({ kind: 'create' });
    await applyProjectLifecycleReceipt(receipt(pending.operationId, {
      kind: 'create',
      oldProjectInstanceId: null,
      newProjectInstanceId: null,
      phase: 'registryCommitted',
      invalidation: { project: false, registry: true },
    }), 'event', dependencies());

    now += PROJECT_LIFECYCLE_SETTLEMENT_TTL_MS + 1;
    registerPendingProjectLifecycleOperation({ kind: 'create', operationId: 'after-event-ttl' });

    expect(getProjectLifecycleRegistrySizeForTests()).toBe(1);
  });

  it('treats a late duplicate of an expired event-only completion as stale with zero effects', async () => {
    let now = 1_000;
    setProjectLifecycleClockForTests(() => now);
    const pending = registerPendingProjectLifecycleOperation({ kind: 'create' });
    const result = receipt(pending.operationId, {
      kind: 'create',
      oldProjectInstanceId: null,
      newProjectInstanceId: null,
      phase: 'registryCommitted',
      invalidation: { project: false, registry: true },
    });
    const first = dependencies();
    await applyProjectLifecycleReceipt(result, 'event', first);
    now += PROJECT_LIFECYCLE_SETTLEMENT_TTL_MS + 1;
    const late = dependencies();

    await expect(applyProjectLifecycleReceipt(result, 'event', late)).resolves.toMatchObject({
      status: 'stale',
    });

    expect(first.refreshRegistry).toHaveBeenCalledOnce();
    expect(late.refreshRegistry).not.toHaveBeenCalled();
    expect(late.prepareProjectTransition).not.toHaveBeenCalled();
  });

  it('reuses an expired operation ID without allowing the old event to claim the new generation', async () => {
    let now = 1_000;
    setProjectLifecycleClockForTests(() => now);
    const operationId = 'reused-operation';
    registerPendingProjectLifecycleOperation({ kind: 'saveAs', operationId });
    await recoverProjectLifecycleDirectFailure(operationId);
    now += PROJECT_LIFECYCLE_SETTLEMENT_TTL_MS + 1;
    registerPendingProjectLifecycleOperation({ kind: 'saveAs', operationId });
    const deps = dependencies();

    await expect(applyProjectLifecycleReceipt(
      receipt(operationId, { path: 'C:/old/metadata.yssbi' }),
      'event',
      deps,
    )).resolves.toMatchObject({ status: 'stale' });
    await expect(applyProjectLifecycleReceipt(
      receipt(operationId, { path: 'C:/new/metadata.yssbi' }),
      'direct',
      deps,
    )).resolves.toMatchObject({ status: 'applied' });
    await expect(applyProjectLifecycleReceipt(
      receipt(operationId, { path: 'C:/new/metadata.yssbi' }),
      'event',
      deps,
    )).resolves.toMatchObject({ status: 'duplicate' });

    expect(deps.prepareProjectTransition).toHaveBeenCalledOnce();
    expect(deps.refreshRegistry).toHaveBeenCalledOnce();
  });

  it('keeps the registry bounded without silently evicting current pending operations', () => {
    for (let index = 0; index < 128; index += 1) {
      registerPendingProjectLifecycleOperation({
        kind: 'create',
        operationId: `operation-${index}`,
      });
    }

    expect(() => registerPendingProjectLifecycleOperation({
      kind: 'create',
      operationId: 'operation-overflow',
    })).toThrow(ProjectLifecycleProtocolError);
    expect(getProjectLifecycleRegistrySizeForTests()).toBe(128);
  });
});
