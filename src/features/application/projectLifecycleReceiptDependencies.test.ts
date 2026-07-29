import { beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useResourceStore } from '@/features/core/resource';
import { ProjectService } from '@/services/project/projectService';
import type { LifecycleMutationResultDto, ProjectRecordRow } from '@/shared/types/dto/project';
import {
  applyProjectLifecycleReceipt,
  getProjectLifecycleOperationForTests,
  registerPendingProjectLifecycleOperation,
  resetProjectLifecycleReceiptHandlerForTests,
} from './projectLifecycleReceipt';
import { createProjectLifecycleReceiptDependencies } from './projectLifecycleReceiptDependencies';
import { logger } from '@/utils/appLogger';

const projectA = '00000000-0000-0000-0000-000000000601';
const projectB = '00000000-0000-0000-0000-000000000602';

function record(): ProjectRecordRow {
  return {
    id: 'record-b',
    name: 'Project B',
    path: 'C:/project-b/metadata.yssbi',
    createdAt: '2026-07-29T00:00:00Z',
    lastOpenedAt: null,
    isFavorite: false,
    rootIdentity: 'native-id',
  };
}

function result(
  operationId: string,
  outcome: 'committed' | 'activationFailed',
): LifecycleMutationResultDto {
  return {
    operationId,
    kind: 'saveAs',
    oldProjectInstanceId: projectA,
    newProjectInstanceId: outcome === 'committed' ? projectB : null,
    phase: outcome === 'committed' ? 'authorityCommitted' : 'registryCommitted',
    outcome,
    record: record(),
    path: record().path,
    recovery: outcome === 'committed' ? null : {
      required: true,
      action: 'activateDestination',
      path: record().path,
      identity: null,
    },
    invalidation: { project: true, registry: true },
  };
}

function index(projectInstanceId: string, publicationRevision: number) {
  return {
    projectInstanceId,
    publicationRevision,
    history: { canUndo: false, canRedo: false },
    projectName: 'Project',
    graphs: [],
    variables: [],
    worksheets: [],
    exportTime: '2026-07-29T00:00:00Z',
    appVersion: '0.2.7',
  };
}

describe('production project lifecycle hydration dependency', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    resetProjectLifecycleReceiptHandlerForTests();
    useProjectIOStore.setState({
      projectInstanceId: projectA,
      currentPath: 'C:/project-a/metadata.yssbi',
    });
    projectPublicationCoordinator.startProject(projectA, 4);
    vi.spyOn(ProjectService, 'getProjectPath').mockResolvedValue('C:/authoritative/metadata.yssbi');
    vi.spyOn(ProjectService, 'getDatabasesVariables').mockResolvedValue({
      databases: {},
      variables: {},
    });
    vi.spyOn(ProjectService, 'listRegisteredProjects').mockResolvedValue([record()]);
  });

  it.each([
    ['database', () => useDatabaseStore.subscribe],
    ['variable', () => useVariableStore.subscribe],
    ['function', () => useGraphMetaStore.subscribe],
    ['layout', () => useLayoutStore.subscribe],
    ['projectIO', () => useProjectIOStore.subscribe],
  ] as const)(
    'completes a coherent receipt transition when a %s listener throws',
    async (_label, getSubscribe) => {
      vi.spyOn(logger.sys, 'error').mockImplementation(() => undefined);
      vi.spyOn(ProjectService, 'getProjectIndex').mockResolvedValue({
        ...index(projectB, 9),
        graphs: [{
          path: 'functions/Add.yssbi-function',
          name: 'Add',
          type: 'function',
          functionRevision: 3,
          functionSignature: { parameters: [], return_type: null },
        }],
      });
      vi.spyOn(ProjectService, 'getDatabasesVariables').mockResolvedValue({
        databases: { 'db-new': { id: 'db-new', name: 'New database' } },
        variables: {},
      });
      useDatabaseStore.setState({ databases: { old: { id: 'old', name: 'Old database' } } });
      useVariableStore.setState({ variables: {}, revisions: { old: 1 } });
      useGraphMetaStore.setState({ graphs: {} });
      let unsubscribe: () => void = () => undefined;
      unsubscribe = getSubscribe()(() => {
        unsubscribe();
        throw new Error('injected listener failure');
      });
      const pending = registerPendingProjectLifecycleOperation({ kind: 'saveAs' });

      await expect(applyProjectLifecycleReceipt(
        result(pending.operationId, 'committed'),
        'direct',
        createProjectLifecycleReceiptDependencies(),
      )).resolves.toMatchObject({ status: 'applied' });

      expect(useDatabaseStore.getState().databases).toEqual({
        'db-new': expect.objectContaining({ id: 'db-new', name: 'New database' }),
      });
      expect(useVariableStore.getState()).toMatchObject({ variables: {}, revisions: {} });
      expect(useGraphMetaStore.getState().graphs).toHaveProperty('functions/Add.yssbi-function');
      expect(useResourceStore.getState().graphOrder).toEqual(['functions/Add.yssbi-function']);
      expect(useProjectIOStore.getState().projectInstanceId).toBe(projectB);
      expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
        projectInstanceId: projectB,
        appliedRevision: 9,
      });
      expect(getProjectLifecycleOperationForTests(pending.operationId)).toMatchObject({
        state: 'complete',
      });
    },
  );

  it.each([
    ['committed', projectB, 0],
    ['activationFailed', projectA, 4],
  ] as const)(
    'prepares and commits %s hydration under receipt-owned transition',
    async (outcome, authoritativeProject, revision) => {
      vi.spyOn(ProjectService, 'getProjectIndex').mockResolvedValue(
        index(authoritativeProject, revision),
      );
      const pending = registerPendingProjectLifecycleOperation({ kind: 'saveAs' });

      const settlement = await applyProjectLifecycleReceipt(
        result(pending.operationId, outcome),
        'direct',
        createProjectLifecycleReceiptDependencies(),
      );

      expect(settlement.status).toBe('applied');
      expect(useProjectIOStore.getState().projectInstanceId).toBe(authoritativeProject);
      expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
        projectInstanceId: authoritativeProject,
        appliedRevision: revision,
      });
      expect(getProjectLifecycleOperationForTests(pending.operationId)).toMatchObject({
        state: 'complete',
      });
    },
  );
});
