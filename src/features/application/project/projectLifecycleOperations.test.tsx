// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import {
  claimProjectLifecycleNotification,
  registerPendingProjectLifecycleOperation,
  resetProjectLifecycleReceiptHandlerForTests,
} from '@/features/application/projectLifecycleReceipt';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { uiStore } from '@/features/core/ui/UIStore';
import { ProjectLifecycleCommittedHandler } from '@/features/core/sync/handlers/ProjectEventHandler';
import { ProjectService } from '@/services/project/projectService';
import type {
  LifecycleMutationOutcome,
  LifecycleMutationResultDto,
  ProjectRecordRow,
} from '@/shared/types/dto/project';
import { useProjectOperations } from '@/features/application/editor/useProjectOperations';
import { useProjectPicker } from './useProjectPicker';
import { saveAllDirtyGraphs } from '@/features/application/editor/saveAllDirtyGraphs';
import { logger } from '@/utils/appLogger';
import {
  installCoreApplicationTestPorts,
  resetCoreApplicationTestPorts,
} from '@/features/application/testHelpers/coreApplicationPorts';
import { applyProjectLifecycleReceipt } from '@/features/application/projectLifecycleReceipt';
import { createProjectLifecycleReceiptDependencies } from '@/features/application/projectLifecycleReceiptDependencies';

const navigate = vi.fn();

vi.mock('react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-router')>()),
  useNavigate: () => navigate,
}));
vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/features/application/editor/saveAllDirtyGraphs', () => ({
  saveAllDirtyGraphs: vi.fn(async () => true),
}));
vi.mock('@/features/core/execution', () => ({
  revokeAllPinPreviewLeases: vi.fn(),
  useExecutionStore: { getState: vi.fn(), setState: vi.fn() },
  getExecutionEventGraph: vi.fn(),
  resolveExecutionGraphPath: vi.fn(),
  graphHasClearableArtifacts: vi.fn(),
  enqueueLiveExecutionEvent: vi.fn(),
}));
vi.mock('@/features/application/execution/openInspectableSource', () => ({
  openWindowInspectableSource: vi.fn(),
}));
vi.mock('@/features/application/graphDiagnostics/warnCallFunctionIssues', () => ({
  warnCallFunctionIssuesBeforeSave: vi.fn(),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function record(id = 'record-b', path = 'C:/project-b/metadata.yssbi'): ProjectRecordRow {
  return {
    id,
    name: 'Project B',
    path,
    createdAt: '2026-07-29T00:00:00Z',
    lastOpenedAt: null,
    isFavorite: false,
    rootIdentity: 'native-id',
  };
}

function saveAsReceipt(
  operationId: string,
  outcome: LifecycleMutationOutcome = 'committed',
): LifecycleMutationResultDto {
  return {
    operationId,
    kind: 'saveAs',
    oldProjectInstanceId: 'project-a',
    newProjectInstanceId: outcome === 'committed' ? 'project-b' : null,
    phase: outcome === 'registryFailed' ? 'destinationCommitted' : 'authorityCommitted',
    outcome,
    record: outcome === 'registryFailed' ? null : record(),
    path: 'C:/project-b/metadata.yssbi',
    recovery: outcome === 'committed' ? null : {
      required: true,
      action: outcome === 'activationFailed' ? 'activateDestination' : 'registerDestination',
      path: 'C:/project-b/metadata.yssbi',
      identity: null,
    },
    invalidation: { project: outcome !== 'registryFailed', registry: true },
  };
}

function activeTerminalRowRejectionReceipt(
  operationId: string,
  row: ProjectRecordRow,
): LifecycleMutationResultDto {
  return {
    operationId,
    kind: 'registryCleanup',
    oldProjectInstanceId: null,
    newProjectInstanceId: null,
    phase: 'registryCommitted',
    outcome: 'registryFailed',
    record: row,
    path: null,
    recovery: {
      required: true,
      action: 'cleanupRegistry',
      path: null,
      identity: null,
    },
    invalidation: { project: false, registry: true },
  };
}

function projectReceipt(
  operationId: string,
  kind: 'create' | 'delete',
  options: { active?: boolean; outcome?: LifecycleMutationOutcome; row?: ProjectRecordRow } = {},
): LifecycleMutationResultDto {
  const outcome = options.outcome ?? 'committed';
  return {
    operationId,
    kind,
    oldProjectInstanceId: options.active ? 'project-a' : null,
    newProjectInstanceId: null,
    phase: kind === 'create' ? 'registryCommitted' : 'authorityCommitted',
    outcome,
    record: options.row ?? record(),
    path: kind === 'create' ? record().path : 'C:/deleted-project',
    recovery: outcome === 'committed' ? null : {
      required: true,
      action: outcome === 'registryPending' ? 'removeRegistryRecord' : 'cleanupTombstone',
      path: 'C:/.deleted-project.tombstone',
      identity: 'native-id',
    },
    invalidation: {
      project: kind === 'delete' && Boolean(options.active),
      registry: true,
    },
  };
}

describe('project lifecycle initiating operations', () => {
  let host: HTMLDivElement;
  let root: Root;
  let operations!: ReturnType<typeof useProjectOperations>;
  let picker!: ReturnType<typeof useProjectPicker>;

  beforeEach(async () => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    resetProjectLifecycleReceiptHandlerForTests();
    installCoreApplicationTestPorts({
      syncEvents: {
        applyProjectLifecycleReceipt: async (result, onProjectCleared, dependencies) => {
          await applyProjectLifecycleReceipt(
            result as LifecycleMutationResultDto,
            'event',
            dependencies as Parameters<typeof applyProjectLifecycleReceipt>[2]
              ?? createProjectLifecycleReceiptDependencies(onProjectCleared),
          );
        },
      },
    });
    vi.mocked(saveAllDirtyGraphs).mockResolvedValue(true);
    vi.spyOn(logger.app, 'error').mockImplementation(() => undefined);
    vi.spyOn(ProjectService, 'listRegisteredProjects').mockResolvedValue([]);
    vi.spyOn(ProjectService, 'registerProject').mockResolvedValue(record('project-a', 'C:/project-a/metadata.yssbi'));
    useProjectIOStore.setState({
      currentPath: 'C:/project-a/metadata.yssbi',
      projectInstanceId: 'project-a',
    });
    projectPublicationCoordinator.startProject('project-a', 4);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    function Harness() {
      operations = useProjectOperations();
      picker = useProjectPicker();
      return null;
    }
    await act(async () => {
      root.render(<Harness />);
      await Promise.resolve();
      await Promise.resolve();
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    uiStore.finishProgress();
    resetCoreApplicationTestPorts();
  });

  it('recovers initiating save-as success when event settles before direct transport rejects', async () => {
    const direct = deferred<LifecycleMutationResultDto | null>();
    const saveAs = vi.spyOn(ProjectService, 'saveProjectAs').mockReturnValue(direct.promise);
    vi.spyOn(ProjectService, 'getProjectPath').mockResolvedValue('C:/project-b/metadata.yssbi');
    vi.spyOn(ProjectService, 'getDatabasesVariables').mockResolvedValue({
      databases: {},
      variables: {},
    });
    vi.spyOn(ProjectService, 'getProjectIndex').mockResolvedValue({
      projectInstanceId: 'project-b',
      publicationRevision: 0,
      history: { canUndo: false, canRedo: false },
      projectName: 'Project B',
      graphs: [],
      variables: [],
      worksheets: [],
      databases: [],
      exportTime: '',
    });
    const toast = vi.spyOn(uiStore, 'showToast');

    let completion!: Promise<void>;
    await act(async () => {
      completion = operations.saveGraphAs();
      await Promise.resolve();
    });
    await vi.waitFor(() => expect(saveAs).toHaveBeenCalledOnce());
    const operationId = saveAs.mock.calls[0][1];
    const result = saveAsReceipt(operationId);
    await act(async () => {
      new ProjectLifecycleCommittedHandler().handle({ result: structuredClone(result) });
      await vi.waitFor(() => expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(2));
    });
    await act(async () => {
      direct.reject(new Error('direct response lost'));
      await completion;
    });

    expect(toast).toHaveBeenCalledOnce();
    expect(toast).toHaveBeenCalledWith('项目已另存为：Project B', 'success', 3000);
    expect(claimProjectLifecycleNotification(operationId)).toBe(false);
  });

  it('gives a mismatching direct save-as DTO zero initiating effects', async () => {
    const direct = deferred<LifecycleMutationResultDto | null>();
    const saveAs = vi.spyOn(ProjectService, 'saveProjectAs').mockReturnValue(direct.promise);
    vi.spyOn(ProjectService, 'getProjectPath').mockResolvedValue('C:/project-b/metadata.yssbi');
    vi.spyOn(ProjectService, 'getDatabasesVariables').mockResolvedValue({ databases: {}, variables: {} });
    vi.spyOn(ProjectService, 'getProjectIndex').mockResolvedValue({
      projectInstanceId: 'project-b',
      publicationRevision: 0,
      history: { canUndo: false, canRedo: false },
      projectName: 'Project B',
      graphs: [],
      variables: [],
      worksheets: [],
      databases: [],
      exportTime: '',
    });
    const toast = vi.spyOn(uiStore, 'showToast');

    let completion!: Promise<void>;
    await act(async () => {
      completion = operations.saveGraphAs();
      await Promise.resolve();
    });
    await vi.waitFor(() => expect(saveAs).toHaveBeenCalledOnce());
    const result = saveAsReceipt(saveAs.mock.calls[0][1]);
    await act(async () => {
      new ProjectLifecycleCommittedHandler().handle({ result });
      await vi.waitFor(() => expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(2));
    });
    await act(async () => {
      direct.resolve({ ...result, path: 'C:/conflicting/metadata.yssbi' });
      await completion;
    });

    expect(toast).not.toHaveBeenCalled();
    expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(2);
  });

  it.each(['registryFailed', 'activationFailed'] as const)(
    'shows the initiating save-as recovery warning for %s',
    async (outcome) => {
      vi.spyOn(ProjectService, 'saveProjectAs').mockImplementation(async (_project, operationId) => (
        saveAsReceipt(operationId, outcome)
      ));
      vi.spyOn(ProjectService, 'getProjectPath').mockResolvedValue('C:/project-a/metadata.yssbi');
      vi.spyOn(ProjectService, 'getDatabasesVariables').mockResolvedValue({
        databases: {},
        variables: {},
      });
      vi.spyOn(ProjectService, 'getProjectIndex').mockResolvedValue({
        projectInstanceId: 'project-a',
        publicationRevision: 4,
        history: { canUndo: false, canRedo: false },
        projectName: 'Project A',
        graphs: [],
        variables: [],
        worksheets: [],
        databases: [],
        exportTime: '',
      });
      const toast = vi.spyOn(uiStore, 'showToast');

      await act(async () => {
        await operations.saveGraphAs();
      });

      expect(toast).toHaveBeenCalledWith(
        expect.stringContaining('另存为需要恢复'),
        'warning',
        4000,
      );
    },
  );

  it('reports registry capacity errors inside the create UI boundary', async () => {
    for (let index = 0; index < 128; index += 1) {
      registerPendingProjectLifecycleOperation({
        kind: 'saveAs',
        operationId: `capacity-${index}`,
      });
    }
    const create = vi.spyOn(ProjectService, 'createProject');
    const toast = vi.spyOn(uiStore, 'showToast');

    await act(async () => {
      await picker.createProject('Blocked', 'C:/blocked');
    });

    expect(create).not.toHaveBeenCalled();
    expect(toast).toHaveBeenCalledWith('Too many pending project lifecycle operations', 'error');
  });

  it('reports a current direct transport failure without lifecycle side effects', async () => {
    vi.spyOn(ProjectService, 'saveProjectAs').mockRejectedValue(new Error('transport down'));
    const toast = vi.spyOn(uiStore, 'showToast');
    const registryCalls = vi.mocked(ProjectService.listRegisteredProjects).mock.calls.length;

    await act(async () => {
      await operations.saveGraphAs();
    });

    expect(toast).toHaveBeenCalledWith('另存为失败：transport down', 'error', 3000);
    expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls);
  });

  it('recovers create list, progress, and success toast from event when direct rejects', async () => {
    act(() => {
      projectPublicationCoordinator.cancelProject();
      useProjectIOStore.setState({ projectInstanceId: null, currentPath: null });
    });
    const request = deferred<LifecycleMutationResultDto>();
    const create = vi.spyOn(ProjectService, 'createProject').mockReturnValue(request.promise);
    const created = record('created', 'C:/created/metadata.yssbi');
    vi.mocked(ProjectService.listRegisteredProjects).mockResolvedValue([created]);
    const toast = vi.spyOn(uiStore, 'showToast');
    const progress = vi.spyOn(uiStore, 'updateProgress');

    let completion!: Promise<void>;
    await act(async () => {
      completion = picker.createProject('Created', 'C:/created');
      await Promise.resolve();
    });
    await vi.waitFor(() => expect(create).toHaveBeenCalledOnce());
    const result = projectReceipt(create.mock.calls[0][2], 'create', { row: created });
    const registryCalls = vi.mocked(ProjectService.listRegisteredProjects).mock.calls.length;
    await act(async () => {
      new ProjectLifecycleCommittedHandler().handle({ result });
      await vi.waitFor(() => {
        expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls + 1);
      });
    });
    await act(async () => {
      request.reject(new Error('direct response lost'));
      await completion;
    });

    expect(picker.projects.map((project) => project.id)).toContain('created');
    expect(progress).toHaveBeenCalledWith(expect.objectContaining({ percent: 1 }));
    expect(toast).toHaveBeenCalledOnce();
    expect(toast).toHaveBeenCalledWith('projectPicker.createSuccess', 'success');
    expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls + 1);
  });

  it('recovers inactive delete cleanup warning and list from event when direct rejects', async () => {
    const request = deferred<LifecycleMutationResultDto>();
    const remove = vi.spyOn(ProjectService, 'deleteRegisteredProjectFiles').mockReturnValue(request.promise);
    vi.mocked(ProjectService.listRegisteredProjects).mockResolvedValue([]);
    const toast = vi.spyOn(uiStore, 'showToast');

    let completion!: Promise<void>;
    await act(async () => {
      completion = picker.deleteProjectFiles('inactive-record');
      await Promise.resolve();
    });
    await vi.waitFor(() => expect(remove).toHaveBeenCalledOnce());
    const result = projectReceipt(remove.mock.calls[0][2], 'delete', {
      outcome: 'cleanupPending',
      row: record('inactive-record'),
    });
    const registryCalls = vi.mocked(ProjectService.listRegisteredProjects).mock.calls.length;
    await act(async () => {
      new ProjectLifecycleCommittedHandler().handle({ result });
      await vi.waitFor(() => {
        expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls + 1);
      });
    });
    await act(async () => {
      request.reject(new Error('direct response lost'));
      await completion;
    });

    expect(picker.projects).toEqual([]);
    expect(toast).toHaveBeenCalledOnce();
    expect(toast).toHaveBeenCalledWith(
      'projectPicker.deleteProjectConfirm.success (cleanupTombstone)',
      'warning',
    );
    expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls + 1);
  });

  it.each(['event-first', 'direct-first'] as const)(
    '%s settles an active terminal-row rejection with one error and no authority or list mutation',
    async (order) => {
      const activeRecord = record('active-record', 'C:/project-a/metadata.yssbi');
      vi.mocked(ProjectService.listRegisteredProjects).mockResolvedValue([activeRecord]);
      await act(async () => {
        await picker.refresh();
      });
      await vi.waitFor(() => expect(picker.currentProjectId).toBe('active-record'));
      const request = deferred<LifecycleMutationResultDto>();
      const remove = vi.spyOn(ProjectService, 'deleteRegisteredProjectFiles').mockReturnValue(request.promise);
      const hydrate = vi.spyOn(ProjectService, 'getProjectIndex');
      const toast = vi.spyOn(uiStore, 'showToast');
      const registryCalls = vi.mocked(ProjectService.listRegisteredProjects).mock.calls.length;

      let completion!: Promise<void>;
      await act(async () => {
        completion = picker.deleteProjectFiles('active-record');
        await Promise.resolve();
      });
      await vi.waitFor(() => expect(remove).toHaveBeenCalledOnce());
      const operationId = remove.mock.calls[0][2];
      const result = activeTerminalRowRejectionReceipt(operationId, activeRecord);

      if (order === 'event-first') {
        await act(async () => {
          new ProjectLifecycleCommittedHandler().handle({ result: structuredClone(result) });
          await vi.waitFor(() => {
            expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls + 1);
          });
          request.resolve(result);
          await completion;
        });
      } else {
        await act(async () => {
          request.resolve(result);
          await completion;
        });
      }

      expect(useProjectIOStore.getState()).toMatchObject({
        currentPath: 'C:/project-a/metadata.yssbi',
        projectInstanceId: 'project-a',
      });
      expect(projectPublicationCoordinator.getSnapshotForTests().projectInstanceId).toBe('project-a');
      expect(picker.projects).toEqual([expect.objectContaining({ id: 'active-record' })]);
      expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls + 1);
      expect(hydrate).not.toHaveBeenCalled();
      expect(toast).toHaveBeenCalledOnce();
      expect(toast).toHaveBeenCalledWith(
        'projectPicker.deleteProjectConfirm.failed: cleanupRegistry',
        'error',
      );
      expect(claimProjectLifecycleNotification(operationId)).toBe(false);

      await act(async () => {
        new ProjectLifecycleCommittedHandler().handle({ result: structuredClone(result) });
        await Promise.resolve();
      });
      expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls + 1);
      expect(toast).toHaveBeenCalledOnce();
    },
  );

  it('recovers active delete registry warning and cleared list from event when direct rejects', async () => {
    const activeRecord = record('active-record', 'C:/project-a/metadata.yssbi');
    vi.mocked(ProjectService.listRegisteredProjects).mockResolvedValue([activeRecord]);
    await act(async () => {
      await picker.refresh();
    });
    await vi.waitFor(() => expect(picker.currentProjectId).toBe('active-record'));
    const request = deferred<LifecycleMutationResultDto>();
    const remove = vi.spyOn(ProjectService, 'deleteRegisteredProjectFiles').mockReturnValue(request.promise);
    const toast = vi.spyOn(uiStore, 'showToast');

    let completion!: Promise<void>;
    await act(async () => {
      completion = picker.deleteProjectFiles('active-record');
      await Promise.resolve();
    });
    await vi.waitFor(() => expect(remove).toHaveBeenCalledOnce());
    vi.mocked(ProjectService.listRegisteredProjects).mockResolvedValue([]);
    const result = projectReceipt(remove.mock.calls[0][2], 'delete', {
      active: true,
      outcome: 'registryPending',
      row: activeRecord,
    });
    const registryCalls = vi.mocked(ProjectService.listRegisteredProjects).mock.calls.length;
    await act(async () => {
      new ProjectLifecycleCommittedHandler().handle({ result });
      await vi.waitFor(() => {
        expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls + 1);
      });
    });
    await act(async () => {
      request.reject(new Error('direct response lost'));
      await completion;
    });

    expect(useProjectIOStore.getState().projectInstanceId).toBeNull();
    expect(picker.projects).toEqual([]);
    expect(toast).toHaveBeenCalledOnce();
    expect(toast).toHaveBeenCalledWith(
      'projectPicker.deleteProjectConfirm.success (removeRegistryRecord)',
      'warning',
    );
    expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls + 1);
  });

  it('gives a no-project create completion zero effects after application generation replacement', async () => {
    act(() => {
      projectPublicationCoordinator.cancelProject();
      useProjectIOStore.setState({ projectInstanceId: null, currentPath: null });
    });
    const request = deferred<LifecycleMutationResultDto>();
    const create = vi.spyOn(ProjectService, 'createProject').mockReturnValue(request.promise);
    const toast = vi.spyOn(uiStore, 'showToast');
    const progress = vi.spyOn(uiStore, 'updateProgress');
    const registryCalls = vi.mocked(ProjectService.listRegisteredProjects).mock.calls.length;

    let completion!: Promise<void>;
    await act(async () => {
      completion = picker.createProject('Created', 'C:/created');
      await Promise.resolve();
    });
    await vi.waitFor(() => expect(create).toHaveBeenCalledOnce());
    projectPublicationCoordinator.cancelProject();
    await act(async () => {
      request.resolve({
        operationId: create.mock.calls[0][2],
        kind: 'create',
        oldProjectInstanceId: null,
        newProjectInstanceId: null,
        phase: 'registryCommitted',
        outcome: 'committed',
        record: record('created', 'C:/created/metadata.yssbi'),
        path: 'C:/created/metadata.yssbi',
        recovery: null,
        invalidation: { project: false, registry: true },
      });
      await completion;
    });

    expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls);
    expect(toast).not.toHaveBeenCalled();
    expect(progress).not.toHaveBeenCalledWith(expect.objectContaining({ percent: 1 }));
  });

  it('gives an inactive delete completion zero effects after application generation replacement', async () => {
    const request = deferred<LifecycleMutationResultDto>();
    const remove = vi.spyOn(ProjectService, 'deleteRegisteredProjectFiles').mockReturnValue(request.promise);
    const toast = vi.spyOn(uiStore, 'showToast');
    const registryCalls = vi.mocked(ProjectService.listRegisteredProjects).mock.calls.length;

    let completion!: Promise<void>;
    await act(async () => {
      completion = picker.deleteProjectFiles('inactive-record');
      await Promise.resolve();
    });
    await vi.waitFor(() => expect(remove).toHaveBeenCalledOnce());
    projectPublicationCoordinator.startProject('project-replacement', 0);
    useProjectIOStore.setState({ projectInstanceId: 'project-replacement' });
    await act(async () => {
      request.resolve({
        operationId: remove.mock.calls[0][2],
        kind: 'delete',
        oldProjectInstanceId: null,
        newProjectInstanceId: null,
        phase: 'authorityCommitted',
        outcome: 'committed',
        record: record('inactive-record'),
        path: 'C:/project-b',
        recovery: null,
        invalidation: { project: false, registry: true },
      });
      await completion;
    });

    expect(ProjectService.listRegisteredProjects).toHaveBeenCalledTimes(registryCalls);
    expect(toast).not.toHaveBeenCalled();
  });
});
