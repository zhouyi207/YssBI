import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PinHistoryProjection } from '@/shared/types/ui';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { pinPreviewCacheKey } from './pinResultIndex';
import {
  revokeAllPinPreviewLeases,
  useExecutionStore,
  type PinPreviewLease,
} from './useExecutionStore';


const declaredOutput: PortAddressDto = {
  kind: 'declared',
  nodeId: 'node-1',
  portKey: 'result',
};

const instanceOutput: PortAddressDto = {
  kind: 'instance',
  nodeId: 'node-1',
  templateKey: 'result',
  instanceId: 'instance-7',
};

function beginPreview(
  graphPath: string,
  port: PortAddressDto,
  generation: number,
): PinPreviewLease {
  return useExecutionStore.getState().beginPinPreview(graphPath, port, generation);
}

describe('useExecutionStore pin result lifecycle', () => {
  beforeEach(() => {
    revokeAllPinPreviewLeases();
    useExecutionStore.setState({
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
  });

  it('synchronously revokes the prior same-pin lease without store access on settlement', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();
    const first = store.beginPinPreview(graphPath, declaredOutput, 1);
    const second = store.beginPinPreview(graphPath, declaredOutput, 2);
    const getExecutionState = vi.spyOn(useExecutionStore, 'getState');
    const completePinPreview = vi.spyOn(store, 'completePinPreview');
    const failPinPreview = vi.spyOn(store, 'failPinPreview');

    expect(first.isCurrent()).toBe(false);
    expect(first.complete('result-stale')).toBe(false);
    expect(first.fail('stale failure')).toBe(false);
    expect(getExecutionState).not.toHaveBeenCalled();
    expect(completePinPreview).not.toHaveBeenCalled();
    expect(failPinPreview).not.toHaveBeenCalled();
    expect(second.isCurrent()).toBe(true);
  });

  it('accepts only the newest preview generation for an exact stable address', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();
    const first = beginPreview(graphPath, declaredOutput, 1);
    const second = beginPreview(graphPath, declaredOutput, 2);

    expect(store.completePinPreview(graphPath, declaredOutput, first.generation, 'result-stale')).toBe(false);
    expect(store.completePinPreview(graphPath, instanceOutput, second.generation, 'result-wrong-port')).toBe(false);
    expect(second.complete('result-current')).toBe(true);

    const preview = useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, declaredOutput),
    );
    expect(preview).toMatchObject({
      generation: second.generation,
      status: 'ready',
      resultId: 'result-current',
      port: declaredOutput,
    });
  });

  it('removes only the matching preview generation', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();
    const staleLease = beginPreview(graphPath, declaredOutput, 1);
    const currentLease = beginPreview(graphPath, declaredOutput, 2);

    expect(store.removePinPreview(graphPath, declaredOutput, staleLease.generation)).toBe(false);
    expect(useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, declaredOutput),
    )).toMatchObject({ generation: currentLease.generation, status: 'pending' });
    expect(store.removePinPreview(graphPath, declaredOutput, currentLease.generation)).toBe(true);
    expect(useExecutionStore.getState().getGraph(graphPath).pinPreviews.has(
      pinPreviewCacheKey(graphPath, declaredOutput),
    )).toBe(false);
  });

  it('does not let a completion revive preview state after graph release', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();
    const lease = beginPreview(graphPath, instanceOutput, 1);

    store.releaseGraphExecutionState(graphPath);

    expect(lease.isCurrent()).toBe(false);
    expect(lease.complete('result-stale')).toBe(false);
    expect(useExecutionStore.getState().graphs[graphPath]).toBeUndefined();
  });

  it('tracks the active opaque run ID only for the live run lifecycle', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();

    store.startExecution(graphPath);
    expect(useExecutionStore.getState().getGraph(graphPath).runId).toBeNull();

    store.setActiveRunId(graphPath, '9007199254740993');
    expect(useExecutionStore.getState().getGraph(graphPath).runId).toBe('9007199254740993');

    store.completeExecution(graphPath);
    expect(useExecutionStore.getState().getGraph(graphPath).runId).toBeNull();
  });

  it('keeps ordered run output bounded to the active run lifecycle', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();
    store.startExecution(graphPath);
    store.setActiveRunId(graphPath, '41');

    store.recordRunOutput(graphPath, {
      runId: 'stale-run',
      sequence: 1,
      stream: 'stdout',
      text: 'stale',
      sourceGraphPath: 'events/Main.yssbi-event',
      sourceNodeId: '00000000-0000-0000-0000-000000000002',
      sourcePort: {
        kind: 'declared',
        nodeId: '00000000-0000-0000-0000-000000000002',
        portKey: 'message',
      },
    });
    store.recordRunOutput(graphPath, {
      runId: '41',
      sequence: 1,
      stream: 'stdout',
      text: 'value',
      sourceGraphPath: 'functions/Nested.yssbi-function',
      sourceNodeId: '00000000-0000-0000-0000-000000000002',
      sourcePort: {
        kind: 'declared',
        nodeId: '00000000-0000-0000-0000-000000000002',
        portKey: 'message',
      },
    });
    store.completeExecution(graphPath);

    expect(useExecutionStore.getState().getGraph(graphPath).runOutput).toMatchObject({
      runId: '41',
      projectionDropped: false,
      entries: [{
        sequence: 1,
        text: 'value',
        sourceGraphPath: 'functions/Nested.yssbi-function',
      }],
    });

    store.clearRunOutput(graphPath);
    expect(useExecutionStore.getState().getGraph(graphPath).runOutput)
      .toEqual({ runId: null, entries: [], projectionDropped: false });

    store.recordPinHistory({
      graphPath,
      output: declaredOutput,
      entries: [],
      selectedResultId: null,
    });
    store.clearRunOutput(graphPath);
    expect(useExecutionStore.getState().getGraph(graphPath).pinHistories.size).toBe(1);

    store.clearGraphRunProjections(graphPath);
    expect(useExecutionStore.getState().getGraph(graphPath).runOutput)
      .toEqual({ runId: null, entries: [], projectionDropped: false });
  });

  it('keeps history projections across graph-dirty visual invalidation', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();
    store.recordPinHistory({
      graphPath,
      output: declaredOutput,
      entries: [],
      selectedResultId: null,
    });
    store.completeExecution(graphPath);

    store.markGraphDirty(graphPath);

    const graph = useExecutionStore.getState().graphs[graphPath];
    expect(graph?.graphDirty).toBe(true);
    expect(graph?.pinHistories.size).toBe(1);
  });

  it('clear action removes frontend history projections only', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();
    store.recordPinHistory({
      graphPath,
      output: declaredOutput,
      entries: [],
      selectedResultId: null,
    });

    store.clearGraphRunProjections(graphPath);

    expect(useExecutionStore.getState().graphs[graphPath]?.pinHistories.size).toBe(0);
  });

  it('releases only frontend result projections when a graph tab is fully closed', () => {
    const graphPath = 'events/Main.yssbi-event';
    const projection: PinHistoryProjection = {
      graphPath,
      output: declaredOutput,
      entries: [],
      selectedResultId: null,
    };
    const store = useExecutionStore.getState();
    store.recordPinHistory(projection);

    store.releaseGraphExecutionState(graphPath);

    expect(useExecutionStore.getState().graphs[graphPath]).toBeUndefined();
  });
});
