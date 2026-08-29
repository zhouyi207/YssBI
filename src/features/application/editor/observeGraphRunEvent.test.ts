import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  revokeAllPinPreviewLeases,
  useExecutionStore,
  type PinPreviewLease,
} from '@/features/core/execution';
import type { RunEvent } from '@/shared/types/domain/runEvent';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { pinPreviewCacheKey } from '@/features/core/execution/pinResultIndex';
import { cancelActiveGraphRun } from './cancelActiveGraphRun';
import {
  observeGraphRunEvent,
  type GraphRunOutcomeState,
  type PinPreviewObservation,
} from './observeGraphRunEvent';

function event(kind: RunEvent['kind']): RunEvent {
  return {
    run: {
      projectSessionId: 'project-session-1',
      graphPath: 'events/Main.yssbi-event',
      runId: '9007199254740993',
    },
    kind,
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

function previewObservation(
  lease: PinPreviewLease,
  port: PortAddressDto = declaredOutput,
): PinPreviewObservation {
  return {
    projectSessionId: null,
    output: { graphPath: 'events/Main.yssbi-event', port },
    generation: lease.generation,
    runId: null,
    terminal: 'pending',
    stale: false,
    lease,
  };
}

describe('observeGraphRunEvent', () => {
  beforeEach(() => {
    revokeAllPinPreviewLeases();
    useExecutionStore.setState({
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
  });

  it('projects the runStarted opaque ID into the active graph state', () => {
    const graphPath = 'events/Main.yssbi-event';
    const outcome: GraphRunOutcomeState = { outcome: 'success' };
    useExecutionStore.getState().startExecution(graphPath);

    observeGraphRunEvent(graphPath, event({ type: 'runStarted' }), outcome);

    expect(useExecutionStore.getState().getGraph(graphPath).runId).toBe('9007199254740993');
  });

  it.each([
    ['declared', declaredOutput],
    ['dynamic instance', instanceOutput],
  ] as const)('completes a %s preview only from its exact pinPreviewResultReady', (_name, port) => {
    const graphPath = 'events/Main.yssbi-event';
    const outcome: GraphRunOutcomeState = { outcome: 'success' };
    const lease = beginPreview(graphPath, port, 1);
    const generation = lease.generation;
    const preview = previewObservation(lease, port);

    observeGraphRunEvent(graphPath, event({ type: 'runStarted' }), outcome, preview);
    observeGraphRunEvent(
      graphPath,
      event({
        type: 'pinPreviewResultReady',
        output: { graphPath, port },
        generation,
        resultId: 'result-current',
      }),
      outcome,
      preview,
    );

    expect(useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, port),
    )).toMatchObject({ status: 'ready', resultId: 'result-current' });
  });

  it('ignores mismatched output identity, backend session, run, and stale generation', () => {
    const graphPath = 'events/Main.yssbi-event';
    const outcome: GraphRunOutcomeState = { outcome: 'success' };
    const staleLease = beginPreview(graphPath, declaredOutput, 1);
    const currentLease = beginPreview(graphPath, declaredOutput, 2);
    const staleGeneration = staleLease.generation;
    const currentGeneration = currentLease.generation;
    const stale = previewObservation(staleLease);
    const current = previewObservation(currentLease);

    observeGraphRunEvent(graphPath, event({ type: 'runStarted' }), outcome, stale);
    observeGraphRunEvent(graphPath, event({ type: 'runStarted' }), outcome, current);
    observeGraphRunEvent(
      graphPath,
      event({
        type: 'pinPreviewResultReady',
        output: { graphPath, port: instanceOutput },
        generation: currentGeneration,
        resultId: 'result-wrong-address',
      }),
      outcome,
      current,
    );
    const wrongSession = event({
      type: 'pinPreviewResultReady',
      output: { graphPath, port: declaredOutput },
      generation: currentGeneration,
      resultId: 'result-stale-session',
    });
    wrongSession.run.projectSessionId = 'stale-backend-session';
    observeGraphRunEvent(graphPath, wrongSession, outcome, current);
    const wrongRun = event({
      type: 'pinPreviewResultReady',
      output: { graphPath, port: declaredOutput },
      generation: currentGeneration,
      resultId: 'result-wrong-run',
    });
    wrongRun.run.runId = 'different-run';
    observeGraphRunEvent(graphPath, wrongRun, outcome, current);
    observeGraphRunEvent(
      graphPath,
      event({
        type: 'pinPreviewResultReady',
        output: { graphPath, port: declaredOutput },
        generation: staleGeneration,
        resultId: 'result-stale-generation',
      }),
      outcome,
      stale,
    );

    expect(useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, declaredOutput),
    )).toMatchObject({
      generation: currentGeneration,
      status: 'pending',
      resultId: null,
    });
  });

  it('ignores exact old-run events locally after the same-pin lease is revoked', () => {
    const graphPath = 'events/Main.yssbi-event';
    const outcome: GraphRunOutcomeState = { outcome: 'success' };
    const staleLease = beginPreview(graphPath, declaredOutput, 1);
    beginPreview(graphPath, declaredOutput, 2);
    const stale = previewObservation(staleLease);
    const store = useExecutionStore.getState();
    const completePinPreview = vi.spyOn(store, 'completePinPreview');
    const failPinPreview = vi.spyOn(store, 'failPinPreview');
    const getExecutionState = vi.spyOn(useExecutionStore, 'getState');

    observeGraphRunEvent(graphPath, event({ type: 'runStarted' }), outcome, stale);
    observeGraphRunEvent(graphPath, event({
      type: 'pinPreviewResultReady',
      output: { graphPath, port: declaredOutput },
      generation: 1,
      resultId: 'result-old',
    }), outcome, stale);
    observeGraphRunEvent(graphPath, event({ type: 'runCompleted' }), outcome, stale);

    expect(stale).toMatchObject({ runId: null, terminal: 'pending', stale: false });
    expect(getExecutionState).not.toHaveBeenCalled();
    expect(completePinPreview).not.toHaveBeenCalled();
    expect(failPinPreview).not.toHaveBeenCalled();
  });

  it.each([
    [{ type: 'runCompleted' } as const, 'completed'],
    [{ type: 'runErrored', code: 'kernelFailed', phase: null } as const, 'error'],
    [{ type: 'runCancelled' } as const, 'cancelled'],
  ])('keeps preview $type isolated from an active ordinary run', (terminal, expectedTerminal) => {
    const graphPath = 'events/Main.yssbi-event';
    const ordinaryOutcome: GraphRunOutcomeState = { outcome: 'success' };
    const store = useExecutionStore.getState();
    store.startExecution(graphPath);
    store.setActiveRunId(graphPath, 'ordinary-run');
    const lease = beginPreview(graphPath, declaredOutput, 1);
    const generation = lease.generation;
    const preview = previewObservation(lease);

    const previewStarted = event({ type: 'runStarted' });
    previewStarted.run.runId = 'preview-run';
    observeGraphRunEvent(graphPath, previewStarted, ordinaryOutcome, preview);
    const previewResultReady = event({
      type: 'pinPreviewResultReady',
      output: { graphPath, port: declaredOutput },
      generation,
      resultId: 'preview-result',
    });
    previewResultReady.run.runId = 'preview-run';
    observeGraphRunEvent(graphPath, previewResultReady, ordinaryOutcome, preview);
    const previewTerminal = event(terminal);
    previewTerminal.run.runId = 'preview-run';
    observeGraphRunEvent(graphPath, previewTerminal, ordinaryOutcome, preview);

    expect(useExecutionStore.getState().getGraph(graphPath)).toMatchObject({
      status: 'running',
      runId: 'ordinary-run',
    });
    expect(ordinaryOutcome.outcome).toBe('success');
    expect(preview.terminal).toBe(expectedTerminal);
    expect(useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, declaredOutput),
    )).toMatchObject({ status: 'ready', resultId: 'preview-result' });
  });

  it('keeps the ordinary run as the cancellation target after preview runStarted', async () => {
    const graphPath = 'events/Main.yssbi-event';
    const ordinaryOutcome: GraphRunOutcomeState = { outcome: 'success' };
    const store = useExecutionStore.getState();
    store.startExecution(graphPath);
    store.setActiveRunId(graphPath, 'ordinary-run');
    const lease = beginPreview(graphPath, declaredOutput, 1);
    const preview = previewObservation(lease);
    const previewStarted = event({ type: 'runStarted' });
    previewStarted.run.runId = 'preview-run';

    observeGraphRunEvent(graphPath, previewStarted, ordinaryOutcome, preview);
    const cancelGraphRun = vi.fn().mockResolvedValue(true);
    await cancelActiveGraphRun(graphPath, { cancelGraphRun });

    expect(cancelGraphRun).toHaveBeenCalledWith('ordinary-run');
    expect(useExecutionStore.getState().getGraph(graphPath)).toMatchObject({
      status: 'running',
      runId: 'ordinary-run',
    });
  });

  it('classifies canonical terminal events', () => {
    const outcome: GraphRunOutcomeState = { outcome: 'success' };

    observeGraphRunEvent(
      'events/Main.yssbi-event',
      event({ type: 'runErrored', code: 'kernelFailed', phase: null }),
      outcome,
    );
    expect(outcome.outcome).toBe('error');

    observeGraphRunEvent('events/Main.yssbi-event', event({ type: 'runCancelled' }), outcome);
    expect(outcome.outcome).toBe('cancelled');
  });
});
