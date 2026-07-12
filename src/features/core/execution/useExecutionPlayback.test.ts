import { describe, expect, it, beforeEach } from 'vitest';
import { useExecutionStore } from './useExecutionStore';
import { resetExecutionVisual } from './executionVisualSession';
import type { RecordedEvent } from '@/shared/types/ui/execution';

const SAMPLE_RECORDING: RecordedEvent[] = [
  { event: { event: 'executionStart' }, timestamp: 0 },
  { event: { event: 'nodeComplete', data: { nodeId: 'n1' } }, timestamp: 1 },
  { event: { event: 'executionComplete', data: { hasError: false } }, timestamp: 2 },
];

describe('replay recording retention', () => {
  beforeEach(() => {
    useExecutionStore.setState({
      graphs: {},
      isPlaying: false,
      playbackGraphPath: null,
    });
  });

  it('startExecution clears recording (live run only)', () => {
    const graphPath = 'g1';
    useExecutionStore.getState().setRecording(graphPath, SAMPLE_RECORDING);
    useExecutionStore.getState().startExecution(graphPath);
    expect(useExecutionStore.getState().getGraph(graphPath).recording).toHaveLength(0);
  });

  it('replay executionStart path keeps recording for repeat playback', () => {
    const graphPath = 'g1';
    useExecutionStore.getState().setRecording(graphPath, SAMPLE_RECORDING);
    useExecutionStore.getState().resetGraphVisuals(graphPath);
    resetExecutionVisual(graphPath);

    expect(useExecutionStore.getState().getGraph(graphPath).recording).toEqual(SAMPLE_RECORDING);
  });
});
