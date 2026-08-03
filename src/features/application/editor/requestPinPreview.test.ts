import { beforeEach, describe, expect, it, vi } from 'vitest';
import { portAddressKey } from '@/features/domain/editorProjection';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  markResourceLoaded,
  useDocumentStateStore,
} from '@/features/core/resource';
import { pinPreviewCacheKey, useExecutionStore } from '@/features/core/execution';
import { uiStore } from '@/features/core/ui/UIStore';
import { ProjectService } from '@/services/project/projectService';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import type { ExecutionDemandDto } from '@/shared/types/dto/executionDemand';
import type { RunEvent } from '@/shared/types/dto/runEvent';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { requestPinPreview } from './requestPinPreview';

const eventGraphPath = 'events/Main.yssbi-event';
const frontendProjectInstanceId = 'frontend-project-instance-1';
const backendProjectSessionId = 'backend-project-session-1';

function runEvent(kind: RunEvent['kind'], runId = 'run-1'): RunEvent {
  return {
    correlation: {
      projectSessionId: backendProjectSessionId,
      graphPath: eventGraphPath,
      graphRevision: '1',
      registryFingerprint: 'registry-1',
      resourceVersions: {},
      compileId: 'compile-1',
      selectionDigest: 'selection-1',
      runId,
      nodeId: null,
      nodeTypeId: null,
      parentCall: null,
    },
    basis: {
      graphRevision: '1',
      registryFingerprint: 'registry-1',
      resourceVersions: {},
    },
    kind,
  };
}

function installGraph(
  graphPath = eventGraphPath,
  outputAddress?: PortAddressDto,
): { outputKey: string; outputAddress: PortAddressDto; inputKey: string } {
  const fixture = makeEditorProjectionFixture({ graphPath });
  if (outputAddress) {
    fixture.projection.nodes[0].ports[0].address = outputAddress;
    fixture.projection.nodes[0].ports[0].templateKey = outputAddress.kind === 'declared'
      ? outputAddress.portKey
      : outputAddress.templateKey;
    fixture.projection.connections[0].output = outputAddress;
  }
  const applied = useGraphDataStore.getState().replaceProjection(
    graphPath,
    fixture.projection,
    1,
  );
  expect(applied.applied).toBe(true);
  const kind = graphPath.startsWith('events/') ? 'event' : 'function';
  markResourceLoaded({ id: graphPath, kind });
  useGraphSessionStore.getState().setFocusedSession('editor-a', graphPath);
  return {
    outputKey: portAddressKey(outputAddress ?? fixture.outputAddress),
    outputAddress: outputAddress ?? fixture.outputAddress,
    inputKey: fixture.inputKey,
  };
}

function emitSuccessfulPreview(
  demand: ExecutionDemandDto,
  onEvent?: (event: RunEvent) => void,
  sourceId = 'source-1',
): void {
  if (demand.type !== 'outputs') throw new Error('expected output demand');
  onEvent?.(runEvent({ type: 'runStarted' }));
  onEvent?.(runEvent({
    type: 'outputReady',
    output: demand.outputs[0],
    sourceId,
  }));
  onEvent?.(runEvent({ type: 'runCompleted' }));
}

describe('requestPinPreview', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    clearProjectLifecycle();
    startProjectLifecycle(frontendProjectInstanceId);
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphSessionStore.getState().reset();
    useDocumentStateStore.getState().clear();
    useExecutionStore.setState({
      previewGeneration: 0,
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
    vi.spyOn(uiStore, 'showToast').mockImplementation(() => undefined);
  });

  it.each([
    {
      name: 'declared',
      address: {
        kind: 'declared',
        nodeId: 'local-node',
        portKey: 'local-out',
      } as const,
    },
    {
      name: 'dynamic instance',
      address: {
        kind: 'instance',
        nodeId: 'local-node',
        templateKey: 'values',
        instanceId: 'instance-7',
      } as const,
    },
  ])('settles the exact projected $name preview when frontend and backend project identities differ', async ({ address }) => {
    const { outputKey } = installGraph(eventGraphPath, address);
    const execute = vi.spyOn(ProjectService, 'executeGraphDocument').mockImplementation(
      async (_graphPath, demand, onEvent) => {
        emitSuccessfulPreview(demand, onEvent);
        return { runId: 'run-1' };
      },
    );

    await expect(requestPinPreview(eventGraphPath, outputKey)).resolves.toMatchObject({
      status: 'completed',
    });

    expect(execute).toHaveBeenCalledWith(
      eventGraphPath,
      {
        type: 'outputs',
        outputs: [{ graphPath: eventGraphPath, port: address }],
        includeDefaultResults: false,
      },
      expect.any(Function),
    );
    expect(useExecutionStore.getState().getGraph(eventGraphPath).pinPreviews.get(
      pinPreviewCacheKey(eventGraphPath, address),
    )).toMatchObject({ status: 'ready', sourceId: 'source-1' });
  });

  it.each([
    {
      name: 'nested function graph',
      prepare: () => {
        const graphPath = 'functions/Helper.yssbi-function';
        const graph = installGraph(graphPath);
        return { graphPath, pinId: graph.outputKey, reason: 'nested-function' } as const;
      },
    },
    {
      name: 'input pin',
      prepare: () => {
        const graph = installGraph();
        return { graphPath: eventGraphPath, pinId: graph.inputKey, reason: 'input-pin' } as const;
      },
    },
    {
      name: 'control output',
      prepare: () => {
        const graph = installGraph();
        useGraphDataStore.getState().graphEntities[eventGraphPath].pins[graph.outputKey].kind = 'control';
        return { graphPath: eventGraphPath, pinId: graph.outputKey, reason: 'non-data-output' } as const;
      },
    },
    {
      name: 'effect output',
      prepare: () => {
        const graph = installGraph();
        useGraphDataStore.getState().graphEntities[eventGraphPath].pins[graph.outputKey].kind = 'effect';
        return { graphPath: eventGraphPath, pinId: graph.outputKey, reason: 'non-data-output' } as const;
      },
    },
    {
      name: 'orphan output',
      prepare: () => {
        const graph = installGraph();
        useGraphDataStore.getState().graphEntities[eventGraphPath].pins[graph.outputKey].orphan = true;
        return { graphPath: eventGraphPath, pinId: graph.outputKey, reason: 'orphan-pin' } as const;
      },
    },
    {
      name: 'missing pin',
      prepare: () => {
        installGraph();
        return { graphPath: eventGraphPath, pinId: 'missing-pin', reason: 'missing-pin' } as const;
      },
    },
    {
      name: 'missing projected address',
      prepare: () => {
        const graph = installGraph();
        delete useGraphDataStore.getState().graphEntities[eventGraphPath].pins[graph.outputKey].address;
        return { graphPath: eventGraphPath, pinId: graph.outputKey, reason: 'missing-address' } as const;
      },
    },
    {
      name: 'missing focused graph session',
      prepare: () => {
        const graph = installGraph();
        useGraphSessionStore.getState().reset();
        return { graphPath: eventGraphPath, pinId: graph.outputKey, reason: 'missing-session' } as const;
      },
    },
    {
      name: 'unloaded graph resource',
      prepare: () => {
        const graph = installGraph();
        useDocumentStateStore.getState().clear();
        return { graphPath: eventGraphPath, pinId: graph.outputKey, reason: 'missing-resource' } as const;
      },
    },
    {
      name: 'missing graph projection',
      prepare: () => {
        useGraphSessionStore.getState().setFocusedSession('editor-a', eventGraphPath);
        markResourceLoaded({ id: eventGraphPath, kind: 'event' });
        return { graphPath: eventGraphPath, pinId: 'missing', reason: 'missing-resource' } as const;
      },
    },
    {
      name: 'stale project lifecycle',
      prepare: () => {
        const graph = installGraph();
        clearProjectLifecycle();
        return {
          graphPath: eventGraphPath,
          pinId: graph.outputKey,
          reason: 'stale-project-lifecycle',
        } as const;
      },
    },
  ])('rejects $name before IPC', async ({ prepare }) => {
    const execute = vi.spyOn(ProjectService, 'executeGraphDocument');
    const request = prepare();

    await expect(requestPinPreview(request.graphPath, request.pinId)).resolves.toEqual({
      status: 'rejected',
      reason: request.reason,
    });

    expect(execute).not.toHaveBeenCalled();
    expect(uiStore.showToast).toHaveBeenCalledOnce();
  });

  it.each([
    {
      name: 'projection object replacement',
      replace: () => {
        const current = useGraphDataStore.getState().graphEntities[eventGraphPath];
        useGraphDataStore.setState({
          graphEntities: {
            ...useGraphDataStore.getState().graphEntities,
            [eventGraphPath]: { ...current, pins: { ...current.pins } },
          },
        });
      },
    },
    {
      name: 'request generation change',
      replace: () => {
        const current = useGraphDataStore.getState().graphEntities[eventGraphPath];
        current.requestGeneration += 1;
      },
    },
    {
      name: 'source revision change',
      replace: () => {
        const current = useGraphDataStore.getState().graphEntities[eventGraphPath];
        current.sourceRevision += 1;
      },
    },
  ])('suppresses pending OutputReady after $name', async ({ replace }) => {
    const { outputKey, outputAddress } = installGraph();
    useExecutionStore.getState().startExecution(eventGraphPath);
    useExecutionStore.getState().setActiveRunId(eventGraphPath, 'ordinary-run');
    vi.spyOn(ProjectService, 'executeGraphDocument').mockImplementation(
      async (_graphPath, demand, onEvent) => {
        if (demand.type !== 'outputs') throw new Error('expected output demand');
        onEvent?.(runEvent({ type: 'runStarted' }));
        replace();
        onEvent?.(runEvent({
          type: 'outputReady',
          output: demand.outputs[0],
          sourceId: 'source-stale-projection',
        }));
        onEvent?.(runEvent({ type: 'runCompleted' }));
        return { runId: 'run-1' };
      },
    );

    await expect(requestPinPreview(eventGraphPath, outputKey)).resolves.toMatchObject({
      status: 'rejected',
    });
    expect(useExecutionStore.getState().getGraph(eventGraphPath).pinPreviews.get(
      pinPreviewCacheKey(eventGraphPath, outputAddress),
    )).toBeUndefined();
    expect(useExecutionStore.getState().getGraph(eventGraphPath)).toMatchObject({
      status: 'running',
      runId: 'ordinary-run',
    });
  });

  it('suppresses an OutputReady after project lifecycle replacement', async () => {
    const { outputKey, outputAddress } = installGraph();
    useExecutionStore.getState().startExecution(eventGraphPath);
    useExecutionStore.getState().setActiveRunId(eventGraphPath, 'ordinary-run');
    vi.spyOn(ProjectService, 'executeGraphDocument').mockImplementation(
      async (_graphPath, demand, onEvent) => {
        if (demand.type !== 'outputs') throw new Error('expected output demand');
        onEvent?.(runEvent({ type: 'runStarted' }));
        startProjectLifecycle('project-session-2');
        onEvent?.(runEvent({
          type: 'outputReady',
          output: demand.outputs[0],
          sourceId: 'source-stale-project',
        }));
        onEvent?.(runEvent({ type: 'runCompleted' }));
        return { runId: 'run-1' };
      },
    );

    await requestPinPreview(eventGraphPath, outputKey);

    expect(useExecutionStore.getState().getGraph(eventGraphPath).pinPreviews.get(
      pinPreviewCacheKey(eventGraphPath, outputAddress),
    )).toBeUndefined();
    expect(useExecutionStore.getState().getGraph(eventGraphPath)).toMatchObject({
      status: 'running',
      runId: 'ordinary-run',
    });
  });

  it.each([
    {
      name: 'project replacement',
      makeStale: () => startProjectLifecycle('project-session-2'),
    },
    {
      name: 'projection replacement',
      makeStale: () => {
        const current = useGraphDataStore.getState().graphEntities[eventGraphPath];
        useGraphDataStore.setState({
          graphEntities: {
            ...useGraphDataStore.getState().graphEntities,
            [eventGraphPath]: { ...current, pins: { ...current.pins } },
          },
        });
      },
    },
  ])('cleans pending preview when command rejection follows $name', async ({ makeStale }) => {
    const { outputKey, outputAddress } = installGraph();
    useExecutionStore.getState().startExecution(eventGraphPath);
    useExecutionStore.getState().setActiveRunId(eventGraphPath, 'ordinary-run');
    const commandError = { code: 'test_stop', message: 'stale after invoke' };
    vi.spyOn(ProjectService, 'executeGraphDocument').mockImplementation(async () => {
      makeStale();
      throw commandError;
    });

    await expect(requestPinPreview(eventGraphPath, outputKey)).resolves.toMatchObject({
      status: 'rejected',
    });
    expect(useExecutionStore.getState().getGraph(eventGraphPath).pinPreviews.get(
      pinPreviewCacheKey(eventGraphPath, outputAddress),
    )).toBeUndefined();
    expect(useExecutionStore.getState().getGraph(eventGraphPath)).toMatchObject({
      status: 'running',
      runId: 'ordinary-run',
    });
  });

  it('does not let stale cleanup remove a newer preview generation for the same pin', async () => {
    const { outputKey, outputAddress } = installGraph();
    const pending: Array<{
      reject: (reason: unknown) => void;
    }> = [];
    vi.spyOn(ProjectService, 'executeGraphDocument').mockImplementation(
      () => new Promise((_resolve, reject) => pending.push({ reject })),
    );

    const staleRequest = requestPinPreview(eventGraphPath, outputKey);
    const previous = useGraphDataStore.getState().graphEntities[eventGraphPath];
    useGraphDataStore.setState({
      graphEntities: {
        ...useGraphDataStore.getState().graphEntities,
        [eventGraphPath]: { ...previous, pins: { ...previous.pins } },
      },
    });
    const currentRequest = requestPinPreview(eventGraphPath, outputKey);
    const currentBeforeCleanup = useExecutionStore.getState().getGraph(eventGraphPath).pinPreviews.get(
      pinPreviewCacheKey(eventGraphPath, outputAddress),
    );
    if (!currentBeforeCleanup) throw new Error('expected newer pending preview');

    pending[0].reject({ code: 'stale_request', message: 'old request rejected' });
    await expect(staleRequest).resolves.toMatchObject({ status: 'rejected' });

    expect(useExecutionStore.getState().getGraph(eventGraphPath).pinPreviews.get(
      pinPreviewCacheKey(eventGraphPath, outputAddress),
    )).toMatchObject({
      generation: currentBeforeCleanup.generation,
      status: 'pending',
    });

    pending[1].reject({ code: 'test_stop', message: 'finish current request' });
    await currentRequest;
  });

  it('suppresses the older completion when two previews race', async () => {
    const { outputKey, outputAddress } = installGraph();
    const callbacks: Array<(event: RunEvent) => void> = [];
    const resolvers: Array<(value: { runId: string }) => void> = [];
    vi.spyOn(ProjectService, 'executeGraphDocument').mockImplementation(
      (_graphPath, _demand, onEvent) => new Promise((resolve) => {
        callbacks.push(onEvent ?? (() => undefined));
        resolvers.push(resolve);
      }),
    );

    const first = requestPinPreview(eventGraphPath, outputKey);
    const second = requestPinPreview(eventGraphPath, outputKey);
    callbacks[0](runEvent({ type: 'runStarted' }, 'run-old'));
    callbacks[1](runEvent({ type: 'runStarted' }, 'run-new'));
    callbacks[1](runEvent({
      type: 'outputReady',
      output: { graphPath: eventGraphPath, port: outputAddress },
      sourceId: 'source-new',
    }, 'run-new'));
    callbacks[0](runEvent({
      type: 'outputReady',
      output: { graphPath: eventGraphPath, port: outputAddress },
      sourceId: 'source-old',
    }, 'run-old'));
    callbacks[0](runEvent({ type: 'runCompleted' }, 'run-old'));
    callbacks[1](runEvent({ type: 'runCompleted' }, 'run-new'));
    resolvers[0]({ runId: 'run-old' });
    resolvers[1]({ runId: 'run-new' });
    await Promise.all([first, second]);

    expect(useExecutionStore.getState().getGraph(eventGraphPath).pinPreviews.get(
      pinPreviewCacheKey(eventGraphPath, outputAddress),
    )).toMatchObject({ status: 'ready', sourceId: 'source-new' });
  });
});
