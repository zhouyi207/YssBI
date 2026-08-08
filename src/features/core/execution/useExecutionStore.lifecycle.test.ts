import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PinResultState } from '@/shared/types/ui';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { pinPreviewCacheKey } from './pinResultIndex';
import {
  revokeAllPinPreviewLeases,
  useExecutionStore,
  type PinPreviewLease,
} from './useExecutionStore';

function samplePinResult(pinId: string, graphPath = 'events/Main.yssbi-event'): PinResultState {
  return {
    graphPath,
    nodeId: 'node-1',
    pinId,
    sourceId: `source-${pinId}`,
    descriptor: {
      sourceId: `source-${pinId}`,
      kind: 'json',
      presentation: { kind: 'inspector' },
      title: 'Result',
    },
  };
}

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
    expect(first.complete('source-stale')).toBe(false);
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

    expect(store.completePinPreview(graphPath, declaredOutput, first.generation, 'source-stale')).toBe(false);
    expect(store.completePinPreview(graphPath, instanceOutput, second.generation, 'source-wrong-port')).toBe(false);
    expect(second.complete('source-current')).toBe(true);

    const preview = useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, declaredOutput),
    );
    expect(preview).toMatchObject({
      generation: second.generation,
      status: 'ready',
      sourceId: 'source-current',
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
    expect(lease.complete('source-stale')).toBe(false);
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

  it('indexes pin results by graphPath and pinId', () => {
    const store = useExecutionStore.getState();
    store.recordPinResult(
      'events/Main.yssbi-event',
      samplePinResult('out-1', 'functions/Helper.yssbi-function'),
    );

    const graph = useExecutionStore.getState().graphs['events/Main.yssbi-event'];
    expect(graph?.pinResults.get('functions/Helper.yssbi-function:out-1')).toBeDefined();
  });

  it('keeps pin results across markGraphDirty while tab session is active', () => {
    const store = useExecutionStore.getState();
    store.recordPinResult('events/Main.yssbi-event', samplePinResult('out-1'));
    store.completeExecution('events/Main.yssbi-event');

    store.markGraphDirty('events/Main.yssbi-event');

    const graph = useExecutionStore.getState().graphs['events/Main.yssbi-event'];
    expect(graph?.graphDirty).toBe(true);
    expect(graph?.pinResults.size).toBe(1);
  });

  it('clears invalidated results across execution buckets', () => {
    const store = useExecutionStore.getState();
    store.recordPinResult(
      'events/Main.yssbi-event',
      samplePinResult('out-fn', 'functions/Helper.yssbi-function'),
    );

    store.clearPinResults('functions/Helper.yssbi-function', ['out-fn']);

    const graph = useExecutionStore.getState().graphs['events/Main.yssbi-event'];
    expect(graph?.pinResults.size).toBe(0);
  });

  it('releases execution state when graph tab is fully closed', () => {
    const store = useExecutionStore.getState();
    store.recordPinResult('events/Main.yssbi-event', samplePinResult('out-1'));

    store.releaseGraphExecutionState('events/Main.yssbi-event');

    expect(useExecutionStore.getState().graphs['events/Main.yssbi-event']).toBeUndefined();
  });
});
