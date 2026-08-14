import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
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

registerCoreApplicationPorts();

const graphPath = 'events/Main.yssbi-event';
const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const output = {
  kind: 'declared' as const,
  nodeId: 'node-1',
  portKey: 'result',
};

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

  it('clears execution state through the shared lifecycle path before its callback', () => {
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
    const onProjectCleared = vi.fn(() => {
      expect(useExecutionStore.getState()).toMatchObject({
        graphs: {},
        playbackGraphPath: null,
        isPlaying: false,
      });
      expect(useProjectIOStore.getState().projectInstanceId).toBeNull();
    });

    new ProjectClearedHandler().handle(undefined, { onProjectCleared });

    expect(lease.isCurrent()).toBe(false);
    expect(cancelProject).toHaveBeenCalledOnce();
    expect(clearProjectData).toHaveBeenCalledOnce();
    expect(onProjectCleared).toHaveBeenCalledOnce();
  });
});
