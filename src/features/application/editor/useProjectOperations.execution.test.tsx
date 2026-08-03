// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ProjectService } from '@/services/project/projectService';
import { uiStore } from '@/features/core/ui/UIStore';
import { useProjectOperations } from './useProjectOperations';

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
  });

  it('passes explicit Default demand for an ordinary event run', async () => {
    await act(async () => {
      await operations.executeGraph();
    });

    expect(ProjectService.executeGraphDocument).toHaveBeenCalledWith(
      'events/Main.yssbi-event',
      { type: 'default' },
      expect.any(Function),
    );
  });
});
