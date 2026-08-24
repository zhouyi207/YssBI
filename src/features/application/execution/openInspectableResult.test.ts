import { beforeEach, describe, expect, it, vi } from 'vitest';

import { openPresentationWindow } from '@/features/application/window';
import { resultPanelKey } from '@/features/domain/result';
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { useExecutionStore } from '@/features/core/execution';
import { ResultService } from '@/services/result/resultService';
import type { ResultDescriptor } from '@/shared/types/dto/result';

const mocks = vi.hoisted(() => ({
  upsertResult: vi.fn(),
  showWorkbenchLayoutError: vi.fn(),
}));

vi.mock('@/features/application/window', () => ({
  openPresentationWindow: vi.fn(),
  presentationWindowPayloadFromDescriptor: vi.fn(() => ({ route: '/plot', windowTitle: 'Plot' })),
}));

vi.mock('@/features/core/dockview/workbenchDockviewPort', () => ({
  workbenchDockviewPort: { upsertResult: mocks.upsertResult },
}));

vi.mock('@/features/application/layout/workbenchLayoutErrorFeedback', () => ({
  showWorkbenchLayoutError: mocks.showWorkbenchLayoutError,
}));

import { openInspectableResult } from './openInspectableResult';

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

beforeEach(() => {
  vi.restoreAllMocks();
  mocks.upsertResult.mockReset();
  mocks.upsertResult.mockResolvedValue({ panelInstanceId: 'result-panel' });
  mocks.showWorkbenchLayoutError.mockReset();
  clearProjectLifecycle();
  startProjectLifecycle('project-1');

  vi.spyOn(ResultService, 'getDescriptor').mockResolvedValue(descriptor);
});

describe('openInspectableResult', () => {
  it('atomically upserts the logical Result panel', async () => {
    await expect(openInspectableResult({ kind: 'result', resultId: '17' }, t))
      .resolves.toBe(true);

    expect(mocks.upsertResult).toHaveBeenCalledOnce();
    expect(mocks.upsertResult).toHaveBeenCalledWith({
      resultKey: resultPanelKey(descriptor),
      resultId: '17',
      title: 'Node result',
      presentation: { kind: 'inspector' },
      source: descriptor.provenance.output,
    });

  });

  it('routes plot descriptors through the same root Result upsert without opening a window', async () => {
    vi.mocked(ResultService.getDescriptor).mockResolvedValueOnce(plotDescriptor);

    await expect(openInspectableResult({ kind: 'result', resultId: '18' }, t))
      .resolves.toBe(true);

    expect(mocks.upsertResult).toHaveBeenCalledWith({
      resultKey: resultPanelKey(plotDescriptor),
      resultId: '18',
      title: 'Scatter result',
      presentation: { kind: 'plot', chart: 'scatter' },
      source: plotDescriptor.provenance.output,
    });
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
    expect(mocks.upsertResult).not.toHaveBeenCalled();
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
    expect(mocks.upsertResult).not.toHaveBeenCalled();
  });

  it('maps root Result upsert failures through typed layout feedback', async () => {
    const failure = new Error('private Dockview failure');
    mocks.upsertResult.mockRejectedValueOnce(failure);

    await expect(openInspectableResult({ kind: 'result', resultId: '17' }, t))
      .resolves.toBe(false);

    expect(mocks.showWorkbenchLayoutError).toHaveBeenCalledOnce();
    expect(mocks.showWorkbenchLayoutError).toHaveBeenCalledWith(failure);
  });
});
