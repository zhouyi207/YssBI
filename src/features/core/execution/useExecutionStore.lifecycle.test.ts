import { beforeEach, describe, expect, it } from 'vitest';
import type { PinResultState } from '@/shared/types/ui';
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

describe('useExecutionStore pin result lifecycle', () => {
  beforeEach(() => {
    useExecutionStore.setState({
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
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
