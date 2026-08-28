import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { useProjectIOStore } from '@/features/application/project/projectIOStore';
import { useExecutionStore } from '@/features/core/execution';
import {
  ComputationSettingsChangedHandler,
  ProjectClearedHandler,
} from './ProjectEventHandler';
import { registerCoreApplicationPorts } from '@/features/application/initialization/registerCoreApplicationPorts';
import {
  installCoreApplicationTestPorts,
  resetCoreApplicationTestPorts,
} from '@/features/application/testHelpers/coreApplicationPorts';
import { logger } from '@/utils/appLogger';
import { projectIOApplicationPort } from '@/features/core/dataStore/projectIOApplicationPort';
import { workbenchLayoutController } from '@/features/application/layout/workbenchLayoutController';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import * as editorBootstrap from '@/features/application/editor/bootstrapEditorGraphSession';
import * as layoutReconcile from '@/features/application/editor/reconcileOpenLayoutTabs';

registerCoreApplicationPorts();

const graphPath = 'events/Main.yssbi-event';
const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const output = {
  kind: 'declared' as const,
  nodeId: 'node-1',
  portKey: 'result',
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe('Project event handlers', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 4);
    useProjectIOStore.setState({
      projectInstanceId,
      currentPath: 'C:/project/metadata.yssbi',
    });
    useExecutionStore.setState({
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
  });

  afterEach(() => {
    resetCoreApplicationTestPorts();
    registerCoreApplicationPorts();
  });

  it('forwards remote computation settings receipts through the application event port', () => {
    const computationSettingsChanged = vi.fn();
    installCoreApplicationTestPorts({
      syncEvents: { computationSettingsChanged },
    });
    const receipt = {
      projectInstanceId,
      operationId: 'settings-operation',
      settingsRevision: 5,
      publicationRevision: 8,
      settings: {
        numeric: { tolerance: { absolute: 1e-10, relative: 1e-7 } },
        missingValues: { statistics: 'reject' as const },
      },
    };

    new ComputationSettingsChangedHandler().handle({ result: receipt });

    expect(computationSettingsChanged).toHaveBeenCalledOnce();
    expect(computationSettingsChanged).toHaveBeenCalledWith(receipt);
  });

  it('gates project readiness around reconcile and bootstraps only the active editor', async () => {
    const callbacks: Array<Parameters<
      typeof workbenchLayoutController.markProjectResourcesReady
    >[0]> = [];
    vi.spyOn(workbenchLayoutController, 'markProjectResourcesReady')
      .mockImplementation((callback) => {
        callbacks.push(callback);
      });
    const reconciliation = deferred<void>();
    const reconcile = vi.spyOn(layoutReconcile, 'reconcileOpenLayoutTabsWithResources')
      .mockImplementation(() => reconciliation.promise as never);
    const activeEditor = vi.spyOn(workbenchDockviewPort, 'getActiveEditorPanel');
    const bootstrap = vi.spyOn(editorBootstrap, 'bootstrapEditorGraphSession')
      .mockResolvedValue(true);
    registerCoreApplicationPorts();

    projectIOApplicationPort().reconcileOpenTabs();
    expect(callbacks).toHaveLength(1);
    let current = true;
    const staleRun = callbacks[0]({ isCurrent: () => current });
    await vi.waitFor(() => expect(reconcile).toHaveBeenCalledOnce());
    current = false;
    reconciliation.resolve();
    await staleRun;
    expect(activeEditor).not.toHaveBeenCalled();
    expect(bootstrap).not.toHaveBeenCalled();

    reconcile.mockResolvedValue(undefined);
    activeEditor.mockReturnValue({
      panelInstanceId: 'editor-active',
      groupId: 'group-active',
      component: 'GraphEditor',
      metadata: {
        role: 'editor',
        resourceRef: graphPath,
        resourceKind: 'event',
      },
      active: true,
      location: { type: 'grid' },
    });
    projectIOApplicationPort().reconcileOpenTabs();
    await callbacks[1]({ isCurrent: () => true });

    expect(bootstrap).toHaveBeenCalledOnce();
    expect(bootstrap).toHaveBeenCalledWith('group-active');
  });

  it('clears execution state through the shared lifecycle path', async () => {
    const execution = useExecutionStore.getState();
    execution.startExecution(graphPath);
    execution.setActiveRunId(graphPath, 'old-run');
    const lease = execution.beginPinPreview(graphPath, output, 1);
    useExecutionStore.setState({
      playbackGraphPath: graphPath,
      isPlaying: true,
    });
    const clearProjectData = vi.spyOn(useProjectIOStore.getState(), 'loadProjectFromData');
    const cancelProject = vi.spyOn(projectPublicationCoordinator, 'cancelProject');

    new ProjectClearedHandler().handle(undefined);
    await vi.waitFor(() => {
      expect(useProjectIOStore.getState().projectInstanceId).toBeNull();
    });

    expect(lease.isCurrent()).toBe(false);
    expect(cancelProject).toHaveBeenCalledOnce();
    expect(clearProjectData).toHaveBeenCalledOnce();
    const clearOwner = projectPublicationCoordinator.getSnapshotForTests();
    expect(clearProjectData.mock.calls[0]).toEqual([
      {
        variables: {},
        graphs: {},
        databases: {},
        metadata: { exportTime: '' },
      },
      null,
      {
        projectInstanceId: clearOwner.projectInstanceId,
        epoch: clearOwner.epoch,
      },
    ]);
    expect(Object.isFrozen(clearProjectData.mock.calls[0][2])).toBe(true);
    expect(useExecutionStore.getState()).toMatchObject({
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
    expect(useProjectIOStore.getState().projectInstanceId).toBeNull();
  });

  it('logs rejected asynchronous clear work without raw exception text', async () => {
    const errorLog = vi.spyOn(logger.sys, 'error').mockImplementation(() => undefined);
    installCoreApplicationTestPorts({
      syncEvents: {
        clearProject: async () => {
          throw new Error('private project clear failure');
        },
      },
    });

    new ProjectClearedHandler().handle(undefined);
    await vi.waitFor(() => expect(errorLog).toHaveBeenCalledOnce());

    expect(errorLog).toHaveBeenCalledWith(
      '[project_lifecycle_protocol_error]',
      'ProjectClearedHandler',
    );
    expect(JSON.stringify(errorLog.mock.calls)).not.toContain('private project clear failure');
  });
});
