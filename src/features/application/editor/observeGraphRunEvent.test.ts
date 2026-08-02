import { beforeEach, describe, expect, it } from 'vitest';
import { useExecutionStore } from '@/features/core/execution';
import type { RunEvent } from '@/shared/types/dto/runEvent';
import { observeGraphRunEvent, type GraphRunOutcomeState } from './observeGraphRunEvent';

function event(kind: RunEvent['kind']): RunEvent {
  return {
    correlation: {
      projectSessionId: 'project-session-1',
      graphPath: 'events/Main.yssbi-event',
      graphRevision: '7',
      registryFingerprint: 'registry-1',
      resourceVersions: {},
      compileId: '9',
      runId: '9007199254740993',
      nodeId: null,
      nodeTypeId: null,
      parentCall: null,
    },
    basis: {
      graphRevision: '7',
      registryFingerprint: 'registry-1',
      resourceVersions: {},
    },
    kind,
  };
}

describe('observeGraphRunEvent', () => {
  beforeEach(() => {
    useExecutionStore.setState({ graphs: {}, playbackGraphPath: null, isPlaying: false });
  });

  it('projects the runStarted opaque ID into the active graph state', () => {
    const graphPath = 'events/Main.yssbi-event';
    const outcome: GraphRunOutcomeState = { outcome: 'success' };
    useExecutionStore.getState().startExecution(graphPath);

    observeGraphRunEvent(graphPath, event({ type: 'runStarted' }), outcome);

    expect(useExecutionStore.getState().getGraph(graphPath).runId).toBe('9007199254740993');
  });

  it('classifies canonical terminal events', () => {
    const outcome: GraphRunOutcomeState = { outcome: 'success' };

    observeGraphRunEvent(
      'events/Main.yssbi-event',
      event({ type: 'runErrored', code: 'kernelFailed' }),
      outcome,
    );
    expect(outcome.outcome).toBe('error');

    observeGraphRunEvent('events/Main.yssbi-event', event({ type: 'runCancelled' }), outcome);
    expect(outcome.outcome).toBe('cancelled');
  });
});
