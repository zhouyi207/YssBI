import { beforeEach, describe, expect, it, vi } from 'vitest';
import { openPresentationWindow } from '@/features/application/window';
import { startProjectLifecycle, clearProjectLifecycle } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { useEditorStore } from '@/features/core/editor';
import { useExecutionStore } from '@/features/core/execution';
import { useResultWorkspaceStore } from '@/features/core/resultWorkspace';
import { ResultService } from '@/services/result/resultService';
import type { ResultDescriptor } from '@/shared/types/dto/result';
import { openInspectableResult } from './openInspectableResult';

vi.mock('@/features/application/window', () => ({
  openPresentationWindow: vi.fn(),
  presentationWindowPayloadFromDescriptor: vi.fn(() => ({ route: '/plot', windowTitle: 'Plot' })),
}));

const descriptor: ResultDescriptor = {
  resultId: '17',
  state: { kind: 'ready' },
  provenance: {
    runId: 'run-1',
    activationId: 'activation-1',
    graphPath: 'events/Main.yssbi-event',
    graphRevision: '1',
    nodeId: 'node-1',
    output: {
      graphPath: 'events/Main.yssbi-event',
      port: { kind: 'declared', nodeId: 'node-1', portKey: 'result' },
    },
    createdAtMs: '1787270400000',
  },
  presentation: { kind: 'inspector' },
  valueKind: 'scalar',
  metadata: null,
  totalCount: null,
  title: 'Node result',
};

const plotDescriptor: ResultDescriptor = {
  ...descriptor,
  resultId: '18',
  presentation: { kind: 'plot', chart: 'scatter' },
  title: 'Scatter result',
};

const t = ((key: string) => key) as never;

describe('openInspectableResult', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    clearProjectLifecycle();
    startProjectLifecycle('project-1');
    useResultWorkspaceStore.getState().reset();
    useEditorStore.setState({ rightSidebarTab: 'details', detailFocus: null });
    vi.spyOn(ResultService, 'getDescriptor').mockResolvedValue(descriptor);
  });

  it('opens a non-plot descriptor in the Result workspace', async () => {
    await expect(openInspectableResult({ kind: 'result', resultId: '17' }, t)).resolves.toBe(true);
    expect(useResultWorkspaceStore.getState().activeTabKey).not.toBeNull();
    expect(useEditorStore.getState().rightSidebarTab).toBe('result');
  });

  it('opens a plot descriptor in the Result workspace without opening a window', async () => {
    vi.mocked(ResultService.getDescriptor).mockResolvedValueOnce(plotDescriptor);

    await expect(openInspectableResult({ kind: 'result', resultId: '18' }, t)).resolves.toBe(true);

    expect(useEditorStore.getState().rightSidebarTab).toBe('result');
    expect(Object.values(useResultWorkspaceStore.getState().tabs)).toContainEqual(
      expect.objectContaining({ resultId: '18', presentation: plotDescriptor.presentation }),
    );
    expect(openPresentationWindow).not.toHaveBeenCalled();
  });

  it('drops Pin history that settles after the project identity changes', async () => {
    let settleHistory!: (value: Awaited<ReturnType<typeof ResultService.getPinHistory>>) => void;
    let markHistoryStarted!: () => void;
    const historyStarted = new Promise<void>((resolve) => {
      markHistoryStarted = resolve;
    });
    vi.spyOn(ResultService, 'getPinHistory').mockImplementationOnce(() => {
      markHistoryStarted();
      return new Promise((resolve) => {
        settleHistory = resolve;
      });
    });
    const recordPinHistory = vi.spyOn(useExecutionStore.getState(), 'recordPinHistory');

    const pending = openInspectableResult({
      kind: 'outputPin',
      graphPath: 'events/Main.yssbi-event',
      output: { kind: 'declared', nodeId: 'node-1', portKey: 'result' },
    }, t);
    await historyStarted;
    clearProjectLifecycle();
    startProjectLifecycle('project-2');
    settleHistory([{
      resultId: '17',
      runId: 'run-1',
      activationId: 'activation-1',
      graphRevision: '1',
      createdAtMs: '1787270400000',
      usage: { kind: 'produced' },
      state: { kind: 'ready' },
    }]);

    await expect(pending).resolves.toBe(false);
    expect(recordPinHistory).not.toHaveBeenCalled();
    expect(ResultService.getDescriptor).not.toHaveBeenCalled();
    expect(useResultWorkspaceStore.getState().order).toEqual([]);
    expect(useEditorStore.getState().rightSidebarTab).toBe('details');
  });

  it('drops a descriptor that settles after the project identity changes', async () => {
    let settleDescriptor!: (value: ResultDescriptor | null) => void;
    let markDescriptorStarted!: () => void;
    const descriptorStarted = new Promise<void>((resolve) => {
      markDescriptorStarted = resolve;
    });
    vi.mocked(ResultService.getDescriptor).mockImplementationOnce(() => {
      markDescriptorStarted();
      return new Promise<ResultDescriptor | null>((resolve) => {
        settleDescriptor = resolve;
      });
    });

    const pending = openInspectableResult({ kind: 'result', resultId: '17' }, t);
    await descriptorStarted;
    clearProjectLifecycle();
    startProjectLifecycle('project-2');
    settleDescriptor(descriptor);

    await expect(pending).resolves.toBe(false);
    expect(useResultWorkspaceStore.getState().order).toEqual([]);
    expect(useEditorStore.getState().rightSidebarTab).toBe('details');
  });
});
