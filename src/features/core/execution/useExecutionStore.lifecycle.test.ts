import { beforeEach, describe, expect, it } from 'vitest';
import type { PinResultState } from '@/shared/types/ui';
import { useExecutionStore } from './useExecutionStore';

function samplePinResult(pinId: string): PinResultState {
  return {
    graphPath: 'events/Main.yssbi-event',
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

  it('keeps pin results across markGraphDirty while tab session is active', () => {
    const store = useExecutionStore.getState();
    store.recordPinResult('events/Main.yssbi-event', samplePinResult('out-1'));
    store.completeExecution('events/Main.yssbi-event');

    store.markGraphDirty('events/Main.yssbi-event');

    const graph = useExecutionStore.getState().graphs['events/Main.yssbi-event'];
    expect(graph?.graphDirty).toBe(true);
    expect(graph?.pinResults.size).toBe(1);
  });

  it('releases execution state when graph tab is fully closed', () => {
    const store = useExecutionStore.getState();
    store.recordPinResult('events/Main.yssbi-event', samplePinResult('out-1'));

    store.releaseGraphExecutionState('events/Main.yssbi-event');

    expect(useExecutionStore.getState().graphs['events/Main.yssbi-event']).toBeUndefined();
  });
});
