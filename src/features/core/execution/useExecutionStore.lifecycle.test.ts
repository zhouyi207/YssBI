import { beforeEach, describe, expect, it } from 'vitest';
import type { PinResultState } from '@/shared/types/ui';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { pinPreviewCacheKey } from './pinResultIndex';
import { useExecutionStore } from './useExecutionStore';

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

describe('useExecutionStore pin result lifecycle', () => {
  beforeEach(() => {
    useExecutionStore.setState({
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
  });

  it('accepts only the newest preview generation for an exact stable address', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();
    const first = store.beginPinPreview(graphPath, declaredOutput);
    const second = store.beginPinPreview(graphPath, declaredOutput);

    expect(store.completePinPreview(graphPath, declaredOutput, first, 'source-stale')).toBe(false);
    expect(store.completePinPreview(graphPath, instanceOutput, second, 'source-wrong-port')).toBe(false);
    expect(store.completePinPreview(graphPath, declaredOutput, second, 'source-current')).toBe(true);

    const preview = useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, declaredOutput),
    );
    expect(preview).toMatchObject({
      generation: second,
      status: 'ready',
      sourceId: 'source-current',
      port: declaredOutput,
    });
  });

  it('removes only the matching preview generation', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();
    const staleGeneration = store.beginPinPreview(graphPath, declaredOutput);
    const currentGeneration = store.beginPinPreview(graphPath, declaredOutput);

    expect(store.removePinPreview(graphPath, declaredOutput, staleGeneration)).toBe(false);
    expect(useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, declaredOutput),
    )).toMatchObject({ generation: currentGeneration, status: 'pending' });
    expect(store.removePinPreview(graphPath, declaredOutput, currentGeneration)).toBe(true);
    expect(useExecutionStore.getState().getGraph(graphPath).pinPreviews.has(
      pinPreviewCacheKey(graphPath, declaredOutput),
    )).toBe(false);
  });

  it('does not let a completion revive preview state after graph release', () => {
    const graphPath = 'events/Main.yssbi-event';
    const store = useExecutionStore.getState();
    const generation = store.beginPinPreview(graphPath, instanceOutput);

    store.releaseGraphExecutionState(graphPath);

    expect(store.completePinPreview(
      graphPath,
      instanceOutput,
      generation,
      'source-stale',
    )).toBe(false);
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
