import { beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { useExecutionStore } from '@/features/core/execution';
import { ProjectClearedHandler } from './ProjectEventHandler';

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
      previewGeneration: 0,
      playbackGraphPath: null,
      isPlaying: false,
    });
  });

  it('clears execution state through the shared lifecycle path before its callback', () => {
    const execution = useExecutionStore.getState();
    execution.startExecution(graphPath);
    execution.setActiveRunId(graphPath, 'old-run');
    execution.beginPinPreview(graphPath, output);
    useExecutionStore.setState({
      playbackGraphPath: graphPath,
      isPlaying: true,
    });
    const clearProjectData = vi.spyOn(useProjectIOStore.getState(), 'loadProjectFromData');
    const cancelProject = vi.spyOn(projectPublicationCoordinator, 'cancelProject');
    const onProjectCleared = vi.fn(() => {
      expect(useExecutionStore.getState()).toMatchObject({
        graphs: {},
        previewGeneration: 0,
        playbackGraphPath: null,
        isPlaying: false,
      });
      expect(useProjectIOStore.getState().projectInstanceId).toBeNull();
    });

    new ProjectClearedHandler().handle(undefined, { onProjectCleared });

    expect(cancelProject).toHaveBeenCalledOnce();
    expect(clearProjectData).toHaveBeenCalledOnce();
    expect(onProjectCleared).toHaveBeenCalledOnce();
  });
});
