import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Variable } from '@/shared/types/domain';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useHistoryStore } from '@/features/core/history';
import { VariableService } from '@/services/variable/variableService';
import { uiStore } from '@/features/core/ui/UIStore';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { ResourceMutationCommittedHandler } from '@/features/core/sync/handlers/ProjectMutationEventHandler';
import { createVariableAction, deleteVariableAction, updateVariableAction } from './variableActions';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => { resolve = settle; });
  return { promise, resolve };
}

const variableResourcePath = 'opaque-variable-path-from-project-index';
const original: Variable = {
  id: '00000000-0000-0000-0000-000000000701',
  name: 'Original',
  dataType: { kind: 'Int64' },
  dataValue: { kind: 'Int64', value: 1 },
  description: '',
  scope: { type: 'global' },
  tags: [],
};

function startProject(projectInstanceId: string): void {
  useProjectIOStore.setState({ projectInstanceId });
  projectPublicationCoordinator.startProject(projectInstanceId, 0);
}

function variableWire(variable: Variable | null): Record<string, unknown> | null {
  if (!variable) return null;
  const dataValue = variable.dataValue as { kind: string; value: unknown };
  return {
    id: variable.id,
    name: variable.name,
    dataType: variable.dataType.kind,
    dataValue: { [dataValue.kind]: dataValue.value },
    description: variable.description,
    scope: variable.scope,
    tags: variable.tags,
  };
}

function mutation(params: {
  revision: number;
  operationId: string;
  before: Variable | null;
  after: Variable | null;
  fromRevision?: number;
  toRevision?: number;
  history?: { canUndo: boolean; canRedo: boolean };
}): ResourceMutationResultDto {
  return {
    operationId: params.operationId,
    projectInstanceId: 'project-a',
    publicationRevision: params.revision,
    moves: [],
    deltas: [{
      resource: { kind: 'variable', key: `variables/${original.id}` },
      fromRevision: params.fromRevision ?? (params.before ? 1 : 0),
      toRevision: params.toRevision ?? (params.before ? 2 : 1),
      causedBy: params.operationId,
      payload: {
        kind: 'variable',
        patch: { before: variableWire(params.before), after: variableWire(params.after) },
      },
    }],
    projectionReplacements: [],
    projectionStatus: { status: 'complete', expectedGraphPaths: [] },
    history: params.history ?? { canUndo: true, canRedo: false },
  };
}

describe('variable command lifecycle guards', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useVariableStore.getState().clear();
    useVariableStore.getState().addVariable(original.id, {
      ...original,
      resourcePath: variableResourcePath,
    });
    useVariableStore.getState().setVariableRevision(original.id, 1);
    startProject('project-a');
  });

  it('does not invoke or publish when the project is replaced inside revision authority read', async () => {
    const authority = useVariableStore.getState();
    vi.spyOn(useVariableStore, 'getState').mockImplementationOnce(() => {
      startProject('project-b');
      return authority;
    });
    const update = vi.spyOn(VariableService, 'updateVariable').mockResolvedValue({
      variableId: original.id,
      variable: { ...original, name: 'Changed' },
      result: null,
    });
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit');
    const toast = vi.spyOn(uiStore, 'showToast');
    const before = {
      variables: structuredClone(authority.variables),
      revisions: structuredClone(authority.revisions),
    };

    await expect(updateVariableAction(original.id, { name: 'Changed' })).resolves.toBeNull();

    expect(update).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    expect(useVariableStore.getState()).toMatchObject(before);
    expect(toast).not.toHaveBeenCalled();
  });

  it('returns without effects when variable revision authority is missing', async () => {
    useVariableStore.getState().setVariableSnapshot({}, {});
    const update = vi.spyOn(VariableService, 'updateVariable');
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit');

    await expect(updateVariableAction(original.id, { name: 'Changed' })).resolves.toBeNull();

    expect(update).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    expect(useVariableStore.getState().variables).toEqual({});
  });

  it('ignores a delayed update completion from the previous project without a toast', async () => {
    const request = deferred<Awaited<ReturnType<typeof VariableService.updateVariable>>>();
    vi.spyOn(VariableService, 'updateVariable').mockReturnValue(request.promise);
    const toast = vi.spyOn(uiStore, 'showToast');

    const completion = updateVariableAction(original.id, { name: 'Changed' });
    startProject('project-b');
    useVariableStore.getState().clear();
    request.resolve({
      variableId: original.id,
      variable: { ...original, name: 'Changed' },
      result: null,
    });

    await expect(completion).resolves.toBeNull();
    expect(useVariableStore.getState().variables).toEqual({});
    expect(toast).not.toHaveBeenCalled();
  });

  it('does not issue the follow-up read after a stale create completion', async () => {
    const request = deferred<Awaited<ReturnType<typeof VariableService.createVariable>>>();
    vi.spyOn(VariableService, 'createVariable').mockReturnValue(request.promise);
    const getVariable = vi.spyOn(VariableService, 'getVariable');
    const toast = vi.spyOn(uiStore, 'showToast');

    const completion = createVariableAction({ activeGraphPath: null, isGlobal: true });
    startProject('project-b');
    useVariableStore.getState().clear();
    request.resolve({
      variableId: original.id,
      variable: original,
      result: mutation({ revision: 1, operationId: crypto.randomUUID(), before: null, after: original }),
    });

    await expect(completion).resolves.toBeNull();
    expect(getVariable).not.toHaveBeenCalled();
    expect(useVariableStore.getState().variables).toEqual({});
    expect(toast).not.toHaveBeenCalled();
  });

  it('deduplicates event-first global update and advances the coordinator watermark once', async () => {
    const operationId = crypto.randomUUID();
    const updated = { ...original, name: 'Changed' };
    const result = mutation({ revision: 1, operationId, before: original, after: updated });
    vi.spyOn(VariableService, 'updateVariable').mockImplementation(async (...args) => {
      expect(args[2]).toBe(1);
      new ResourceMutationCommittedHandler().handle({ result });
      return { variableId: original.id, variable: updated, result };
    });
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit');

    await expect(updateVariableAction(original.id, { name: 'Changed' })).resolves.toEqual({
      ...updated,
      resourcePath: variableResourcePath,
    });
    expect(useVariableStore.getState().variables[original.id]).toEqual({
      ...updated,
      resourcePath: variableResourcePath,
    });
    expect(submit).toHaveBeenCalledTimes(2);
    expect(projectPublicationCoordinator.captureCommandLifecycle().publicationRevision).toBe(1);
  });

  it('preserves index resource metadata for a direct-first update and event echo', async () => {
    const operationId = crypto.randomUUID();
    const updated = { ...original, name: 'Direct changed' };
    const result = mutation({ revision: 1, operationId, before: original, after: updated });
    vi.spyOn(VariableService, 'updateVariable').mockResolvedValue({
      variableId: original.id,
      variable: updated,
      result,
    });

    await expect(updateVariableAction(original.id, { name: updated.name })).resolves.toEqual({
      ...updated,
      resourcePath: variableResourcePath,
    });
    new ResourceMutationCommittedHandler().handle({ result });
    await vi.waitFor(() => expect(
      projectPublicationCoordinator.captureCommandLifecycle().publicationRevision,
    ).toBe(1));
    expect(useVariableStore.getState().variables[original.id]).toEqual({
      ...updated,
      resourcePath: variableResourcePath,
    });
  });

  it('deduplicates direct-first create and event echo without a follow-up read', async () => {
    useVariableStore.getState().clear();
    const created = { ...original };
    const operationId = crypto.randomUUID();
    const result = mutation({ revision: 1, operationId, before: null, after: created });
    vi.spyOn(VariableService, 'createVariable').mockImplementation(async (...args) => {
      expect(args[2]).toBe(0);
      return { variableId: created.id, variable: created, result };
    });
    const getVariable = vi.spyOn(VariableService, 'getVariable');

    await expect(createVariableAction({ activeGraphPath: null, isGlobal: true })).resolves.toBe(created.id);
    new ResourceMutationCommittedHandler().handle({ result });
    await vi.waitFor(() => expect(
      projectPublicationCoordinator.captureCommandLifecycle().publicationRevision,
    ).toBe(1));
    expect(getVariable).not.toHaveBeenCalled();
    expect(useVariableStore.getState().variables[created.id]).toEqual(created);
  });

  it('applies contiguous global create update delete publications across event and direct orderings', async () => {
    useVariableStore.getState().clear();
    const created = { ...original, name: 'Created' };
    const updated = { ...created, name: 'Updated', description: 'metadata' };
    const createResult = mutation({
      revision: 1,
      operationId: crypto.randomUUID(),
      before: null,
      after: created,
      fromRevision: 0,
      toRevision: 1,
    });
    const updateResult = mutation({
      revision: 2,
      operationId: crypto.randomUUID(),
      before: created,
      after: updated,
      fromRevision: 1,
      toRevision: 2,
    });
    const deleteResult = mutation({
      revision: 3,
      operationId: crypto.randomUUID(),
      before: updated,
      after: null,
      fromRevision: 2,
      toRevision: 3,
      history: { canUndo: true, canRedo: false },
    });
    const handler = new ResourceMutationCommittedHandler();

    handler.handle({ result: createResult });
    await projectPublicationCoordinator.submit({ result: createResult });
    await projectPublicationCoordinator.submit({ result: updateResult });
    handler.handle({ result: updateResult });
    handler.handle({ result: deleteResult });
    await projectPublicationCoordinator.submit({ result: deleteResult });

    expect(useVariableStore.getState().variables[original.id]).toBeUndefined();
    expect(useVariableStore.getState().variables[original.id]?.resourcePath).toBeUndefined();
    expect(useVariableStore.getState().revisions[original.id]).toBe(3);
    expect(useHistoryStore.getState()).toMatchObject({ canUndo: true, canRedo: false });
    expect(projectPublicationCoordinator.captureCommandLifecycle().publicationRevision).toBe(3);
  });

  it('passes the authoritative variable revision to delete and applies only publication result', async () => {
    const operationId = crypto.randomUUID();
    const result = mutation({ revision: 1, operationId, before: original, after: null });
    const remove = vi.spyOn(VariableService, 'deleteVariable').mockResolvedValue({
      variableId: original.id,
      variable: null,
      result,
    });

    await expect(deleteVariableAction(original.id)).resolves.toBe(true);
    expect(remove.mock.calls[0][2]).toBe(1);
    expect(useVariableStore.getState().variables[original.id]).toBeUndefined();
  });
});
