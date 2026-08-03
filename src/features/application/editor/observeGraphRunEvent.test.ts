import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useExecutionStore } from '@/features/core/execution';
import type { RunEvent } from '@/shared/types/dto/runEvent';
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
    correlation: {
      projectSessionId: 'project-session-1',
      graphPath: 'events/Main.yssbi-event',
      graphRevision: '7',
      registryFingerprint: 'registry-1',
      resourceVersions: {},
      compileId: '9',
      selectionDigest: 'demand-selection-a',
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

function previewObservation(
  generation: number,
  port: PortAddressDto = declaredOutput,
): PinPreviewObservation {
  return {
    projectSessionId: null,
    output: { graphPath: 'events/Main.yssbi-event', port },
    generation,
    runId: null,
    terminal: 'pending',
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

  it.each([
    ['declared', declaredOutput],
    ['dynamic instance', instanceOutput],
  ] as const)('completes a %s preview only from its exact stable OutputReady', (_name, port) => {
    const graphPath = 'events/Main.yssbi-event';
    const outcome: GraphRunOutcomeState = { outcome: 'success' };
    const generation = useExecutionStore.getState().beginPinPreview(graphPath, port);
    const preview = previewObservation(generation, port);

    observeGraphRunEvent(graphPath, event({ type: 'runStarted' }), outcome, preview);
    observeGraphRunEvent(
      graphPath,
      event({
        type: 'outputReady',
        output: { graphPath, port },
        sourceId: 'source-current',
      }),
      outcome,
      preview,
    );

    expect(useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, port),
    )).toMatchObject({ status: 'ready', sourceId: 'source-current' });
  });

  it('ignores mismatched output identity, backend session, run, and stale generation', () => {
    const graphPath = 'events/Main.yssbi-event';
    const outcome: GraphRunOutcomeState = { outcome: 'success' };
    const staleGeneration = useExecutionStore.getState().beginPinPreview(graphPath, declaredOutput);
    const currentGeneration = useExecutionStore.getState().beginPinPreview(graphPath, declaredOutput);
    const stale = previewObservation(staleGeneration);
    const current = previewObservation(currentGeneration);

    observeGraphRunEvent(graphPath, event({ type: 'runStarted' }), outcome, stale);
    observeGraphRunEvent(graphPath, event({ type: 'runStarted' }), outcome, current);
    observeGraphRunEvent(
      graphPath,
      event({
        type: 'outputReady',
        output: { graphPath, port: instanceOutput },
        sourceId: 'source-wrong-address',
      }),
      outcome,
      current,
    );
    const wrongSession = event({
      type: 'outputReady',
      output: { graphPath, port: declaredOutput },
      sourceId: 'source-stale-session',
    });
    wrongSession.correlation.projectSessionId = 'stale-backend-session';
    observeGraphRunEvent(graphPath, wrongSession, outcome, current);
    const wrongRun = event({
      type: 'outputReady',
      output: { graphPath, port: declaredOutput },
      sourceId: 'source-wrong-run',
    });
    wrongRun.correlation.runId = 'different-run';
    observeGraphRunEvent(graphPath, wrongRun, outcome, current);
    observeGraphRunEvent(
      graphPath,
      event({
        type: 'outputReady',
        output: { graphPath, port: declaredOutput },
        sourceId: 'source-stale-generation',
      }),
      outcome,
      stale,
    );

    expect(useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, declaredOutput),
    )).toMatchObject({
      generation: currentGeneration,
      status: 'pending',
      sourceId: null,
    });
  });

  it.each([
    [{ type: 'runCompleted' } as const, 'completed'],
    [{ type: 'runErrored', code: 'kernelFailed' } as const, 'error'],
    [{ type: 'runCancelled' } as const, 'cancelled'],
  ])('keeps preview $type isolated from an active ordinary run', (terminal, expectedTerminal) => {
    const graphPath = 'events/Main.yssbi-event';
    const ordinaryOutcome: GraphRunOutcomeState = { outcome: 'success' };
    const store = useExecutionStore.getState();
    store.startExecution(graphPath);
    store.setActiveRunId(graphPath, 'ordinary-run');
    const generation = store.beginPinPreview(graphPath, declaredOutput);
    const preview = previewObservation(generation);

    const previewStarted = event({ type: 'runStarted' });
    previewStarted.correlation.runId = 'preview-run';
    observeGraphRunEvent(graphPath, previewStarted, ordinaryOutcome, preview);
    const outputReady = event({
      type: 'outputReady',
      output: { graphPath, port: declaredOutput },
      sourceId: 'preview-source',
    });
    outputReady.correlation.runId = 'preview-run';
    observeGraphRunEvent(graphPath, outputReady, ordinaryOutcome, preview);
    const previewTerminal = event(terminal);
    previewTerminal.correlation.runId = 'preview-run';
    observeGraphRunEvent(graphPath, previewTerminal, ordinaryOutcome, preview);

    expect(useExecutionStore.getState().getGraph(graphPath)).toMatchObject({
      status: 'running',
      runId: 'ordinary-run',
    });
    expect(ordinaryOutcome.outcome).toBe('success');
    expect(preview.terminal).toBe(expectedTerminal);
    expect(useExecutionStore.getState().getGraph(graphPath).pinPreviews.get(
      pinPreviewCacheKey(graphPath, declaredOutput),
    )).toMatchObject({ status: 'ready', sourceId: 'preview-source' });
  });

  it('keeps the ordinary run as the cancellation target after preview runStarted', async () => {
    const graphPath = 'events/Main.yssbi-event';
    const ordinaryOutcome: GraphRunOutcomeState = { outcome: 'success' };
    const store = useExecutionStore.getState();
    store.startExecution(graphPath);
    store.setActiveRunId(graphPath, 'ordinary-run');
    const generation = store.beginPinPreview(graphPath, declaredOutput);
    const preview = previewObservation(generation);
    const previewStarted = event({ type: 'runStarted' });
    previewStarted.correlation.runId = 'preview-run';

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
      event({ type: 'runErrored', code: 'kernelFailed' }),
      outcome,
    );
    expect(outcome.outcome).toBe('error');

    observeGraphRunEvent('events/Main.yssbi-event', event({ type: 'runCancelled' }), outcome);
    expect(outcome.outcome).toBe('cancelled');
  });
});
