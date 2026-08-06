// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ProjectService } from '@/services/project/projectService';
import { uiStore } from '@/features/core/ui/UIStore';
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import type { RunEvent } from '@/shared/types/dto/runEvent';
import { useProjectOperations } from './useProjectOperations';

const projectInstanceId = 'project-instance-1';
const graphPath = 'events/Main.yssbi-event';

function runStartedEvent(): RunEvent {
  return {
    correlation: {
      projectSessionId: 'backend-session-1',
      graphPath,
      graphRevision: '1',
      registryFingerprint: 'registry-1',
      resourceVersions: {},
      compileId: '1',
      selectionDigest: 'selection-1',
      runId: 'run-stale',
      nodeId: null,
      nodeTypeId: null,
      parentCall: null,
    },
    basis: {
      graphRevision: '1',
      registryFingerprint: 'registry-1',
      resourceVersions: {},
    },
    kind: { type: 'runStarted' },
  };
}

const executionState = {
  graphs: {},
  startExecution: vi.fn(),
  setActiveRunId: vi.fn(),
  commitExecutionVisual: vi.fn(),
  setRecording: vi.fn(),
  completeExecution: vi.fn(),
  failExecution: vi.fn(),
  interruptExecution: vi.fn(),
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/features/core/execution', () => ({
  useExecutionStore: { getState: () => executionState },
  resolveExecutionGraphPath: () => 'events/Main.yssbi-event',
  getExecutionEventGraph: () => ({
    graph: { name: 'Main', path: 'events/Main.yssbi-event', type: 'event' },
  }),
  graphHasClearableArtifacts: () => false,
}));
vi.mock('@/features/core/execution/executionRecording', () => ({
  ensureGraphExecutionTerminal: vi.fn(),
}));
vi.mock('@/features/application/graphDiagnostics/warnCallFunctionIssues', () => ({
  warnCallFunctionIssuesBeforeSave: vi.fn(),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

describe('useProjectOperations execution demand', () => {
  let container: HTMLDivElement;
  let root: Root;
  let operations: ReturnType<typeof useProjectOperations>;

  beforeEach(() => {
    vi.clearAllMocks();
    clearProjectLifecycle();
    startProjectLifecycle(projectInstanceId);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.spyOn(uiStore, 'showToast').mockImplementation(() => undefined);
    vi.spyOn(ProjectService, 'executeGraphDocument').mockResolvedValue({ runId: 'run-1' });

    function Harness() {
      operations = useProjectOperations();
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
    clearProjectLifecycle();
  });

  it('passes explicit Default demand for an ordinary event run', async () => {
    await act(async () => {
      await operations.executeGraph();
    });

    expect(ProjectService.executeGraphDocument).toHaveBeenCalledWith(
      projectInstanceId,
      graphPath,
      { type: 'default' },
      expect.any(Function),
    );
  });

  it('ignores delayed events and completion after project lifecycle replacement', async () => {
    let emit!: (event: RunEvent) => void;
    let resolveExecution!: (value: { runId: string }) => void;
    vi.mocked(ProjectService.executeGraphDocument).mockImplementation(
      (_projectInstanceId, _graphPath, _demand, onEvent) => new Promise((resolve) => {
        emit = onEvent ?? (() => undefined);
        resolveExecution = resolve;
      }),
    );

    const execution = operations.executeGraph();
    startProjectLifecycle('project-instance-2');
    emit(runStartedEvent());
    resolveExecution({ runId: 'run-stale' });
    await act(async () => execution);

    expect(executionState.setActiveRunId).not.toHaveBeenCalled();
    expect(executionState.commitExecutionVisual).not.toHaveBeenCalled();
    expect(executionState.completeExecution).not.toHaveBeenCalled();
    expect(executionState.failExecution).not.toHaveBeenCalled();
    expect(executionState.interruptExecution).not.toHaveBeenCalled();
    expect(uiStore.showToast).not.toHaveBeenCalled();
  });
});
