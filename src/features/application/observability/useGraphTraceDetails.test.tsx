// @vitest-environment happy-dom
import { act, StrictMode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TraceDecimalString, TraceSpanDto } from '@/shared/types/dto/trace';
import { TraceService } from '@/services/nodeSystem/traceService';
import { projectTraceSpan, useGraphTraceDetails } from './useGraphTraceDetails';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const projectIdentity = vi.hoisted(() => ({ activeId: 'project-1' }));

vi.mock('@/features/application/projectCommandContext', () => ({
  captureProjectCommandContext: vi.fn(() => {
    const capturedId = projectIdentity.activeId;
    return {
      projectInstanceId: capturedId,
      projectEpoch: 1,
      publicationRevision: 1,
      operationId: 'operation-1',
      operationPendingKey: `${capturedId}:operation-1`,
      isCurrent: () => projectIdentity.activeId === capturedId,
      assertCurrent: vi.fn(),
    };
  }),
}));

vi.mock('@/features/core/dataStore/projectIOStore', () => ({
  useProjectIOStore: <T,>(selector: (state: { projectInstanceId: string }) => T): T => selector({
    projectInstanceId: projectIdentity.activeId,
  }),
}));

vi.mock('@/services/nodeSystem/traceService', () => ({
  TraceService: {
    listGraphTraces: vi.fn(),
    getRunTrace: vi.fn(),
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function trace(
  spanId: TraceDecimalString,
  runId: TraceDecimalString,
  graphPath = 'events/Main.yssbi-event',
  startedAt: TraceDecimalString = '9007199254740993',
  finishedAt: TraceDecimalString = '9007199254741003',
): TraceSpanDto {
  return {
    spanId,
    parentSpanId: null,
    runId,
    operationId: null,
    activationId: null,
    attemptId: null,
    kind: 'run',
    startedAt,
    finishedAt,
    outcome: 'success',
    correlation: {
      projectSessionId: 'project-session-1',
      graphPath,
      graphRevision: '7',
      registryFingerprint: 'registry-fingerprint-1',
      resourceVersions: { dataset: 'version-1' },
      compileId: '10',
      selectionDigest: 'demand-selection-a',
      runId,
      nodeId: null,
      nodeTypeId: null,
      parentCall: null,
    },
  };
}

describe('useGraphTraceDetails', () => {
  let host: HTMLDivElement;
  let root: Root;
  let current: ReturnType<typeof useGraphTraceDetails> | undefined;

  function Harness({ graphPath }: { graphPath: string }) {
    current = useGraphTraceDetails(graphPath);
    return null;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(TraceService.listGraphTraces).mockReset();
    vi.mocked(TraceService.getRunTrace).mockReset();
    projectIdentity.activeId = 'project-1';
    current = undefined;
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  async function renderHook(graphPath = 'events/Main.yssbi-event') {
    await act(async () => {
      root.render(<Harness graphPath={graphPath} />);
      await Promise.resolve();
    });
  }

  it('queries traces for the initial graph with the captured project identity', async () => {
    const records = [trace('1', '11')];
    vi.mocked(TraceService.listGraphTraces).mockResolvedValueOnce(records);

    await renderHook();

    expect(TraceService.listGraphTraces).toHaveBeenCalledWith(
      'project-1',
      'events/Main.yssbi-event',
    );
    expect(current).toMatchObject({
      graphTraces: records,
      graphLoading: false,
      graphError: null,
    });
  });

  it('publishes the current initial query under React Strict Mode', async () => {
    const records = [trace('1', '11')];
    vi.mocked(TraceService.listGraphTraces).mockResolvedValue(records);

    await act(async () => {
      root.render(
        <StrictMode>
          <Harness graphPath="events/Main.yssbi-event" />
        </StrictMode>,
      );
      await Promise.resolve();
    });

    expect(current).toMatchObject({
      graphTraces: records,
      graphLoading: false,
      graphError: null,
    });
  });

  it('refreshes the graph list and discards an older graph completion', async () => {
    const initial = [trace('1', '11')];
    const older = deferred<TraceSpanDto[]>();
    const newest = [trace('3', '13')];
    vi.mocked(TraceService.listGraphTraces)
      .mockResolvedValueOnce(initial)
      .mockReturnValueOnce(older.promise)
      .mockResolvedValueOnce(newest);
    await renderHook();

    act(() => {
      void current?.refresh();
    });
    await act(async () => {
      await current?.refresh();
    });
    expect(current?.graphTraces).toEqual(newest.map(projectTraceSpan));

    await act(async () => older.resolve([trace('2', '12')]));
    expect(current?.graphTraces).toEqual(newest.map(projectTraceSpan));
  });

  it('projects nonnegative bigint durations without changing opaque IDs', async () => {
    const span = trace('9007199254740993', '11', undefined, '90071992547409930', '90071992547410055');
    vi.mocked(TraceService.listGraphTraces).mockResolvedValueOnce([span]);

    await renderHook();

    expect(current?.graphTraces[0]).toMatchObject({
      spanId: '9007199254740993',
      startedAt: '90071992547409930',
      finishedAt: '90071992547410055',
      durationNanos: 125n,
    });
  });

  it('loads a selected run and clears its details when selection is cleared', async () => {
    vi.mocked(TraceService.listGraphTraces).mockResolvedValueOnce([trace('1', '11')]);
    const runRecords = [trace('4', '9007199254740993')];
    vi.mocked(TraceService.getRunTrace).mockResolvedValueOnce(runRecords);
    await renderHook();

    await act(async () => {
      await current?.selectRun('9007199254740993');
    });

    expect(TraceService.getRunTrace).toHaveBeenCalledWith(
      'project-1',
      '9007199254740993',
    );
    expect(current).toMatchObject({
      selectedRunId: '9007199254740993',
      runTrace: runRecords,
      runLoading: false,
      runError: null,
    });

    await act(async () => {
      await current?.selectRun(null);
    });
    expect(current).toMatchObject({
      selectedRunId: null,
      runTrace: [],
      runLoading: false,
      runError: null,
    });
  });

  it('suppresses stale-project completions from graph and run queries', async () => {
    const graphRequest = deferred<TraceSpanDto[]>();
    const runRequest = deferred<TraceSpanDto[]>();
    vi.mocked(TraceService.listGraphTraces)
      .mockReturnValueOnce(graphRequest.promise)
      .mockResolvedValueOnce([]);
    vi.mocked(TraceService.getRunTrace).mockReturnValueOnce(runRequest.promise);
    await renderHook();

    act(() => {
      void current?.selectRun('21');
    });
    projectIdentity.activeId = 'project-2';
    await act(async () => {
      graphRequest.resolve([trace('1', '21')]);
      runRequest.resolve([trace('2', '21')]);
      await Promise.all([graphRequest.promise, runRequest.promise]);
    });

    expect(current?.graphTraces).toEqual([]);
    expect(current?.runTrace).toEqual([]);
    expect(current?.graphError).toBeNull();
    expect(current?.runError).toBeNull();
    expect(current?.graphLoading).toBe(false);
    expect(current?.runLoading).toBe(false);
  });

  it('releases loading without publishing stale-project graph or run rejections', async () => {
    const graphRequest = deferred<TraceSpanDto[]>();
    const runRequest = deferred<TraceSpanDto[]>();
    vi.mocked(TraceService.listGraphTraces)
      .mockReturnValueOnce(graphRequest.promise)
      .mockResolvedValueOnce([]);
    vi.mocked(TraceService.getRunTrace).mockReturnValueOnce(runRequest.promise);
    await renderHook();

    act(() => {
      void current?.selectRun('21');
    });
    expect(current).toMatchObject({ graphLoading: true, runLoading: true });

    projectIdentity.activeId = 'project-2';
    await act(async () => {
      graphRequest.reject({ code: 'stale_graph_error', message: 'stale graph failure' });
      runRequest.reject({ code: 'stale_run_error', message: 'stale run failure' });
      await Promise.allSettled([graphRequest.promise, runRequest.promise]);
    });

    expect(current).toMatchObject({
      graphTraces: [],
      graphLoading: false,
      graphError: null,
      runTrace: [],
      runLoading: false,
      runError: null,
    });
  });

  it('re-arms the graph query when project identity changes without remounting', async () => {
    const staleRequest = deferred<TraceSpanDto[]>();
    const currentRecords = [trace('2', '22')];
    vi.mocked(TraceService.listGraphTraces)
      .mockReturnValueOnce(staleRequest.promise)
      .mockResolvedValueOnce(currentRecords);
    await renderHook();

    projectIdentity.activeId = 'project-2';
    await act(async () => {
      root.render(<Harness graphPath="events/Main.yssbi-event" />);
      await Promise.resolve();
    });

    expect(TraceService.listGraphTraces).toHaveBeenCalledTimes(2);
    expect(TraceService.listGraphTraces).toHaveBeenLastCalledWith(
      'project-2',
      'events/Main.yssbi-event',
    );
    expect(current).toMatchObject({
      graphTraces: currentRecords,
      graphLoading: false,
      graphError: null,
    });

    await act(async () => staleRequest.resolve([trace('1', '11')]));
    expect(current?.graphTraces).toEqual(currentRecords.map(projectTraceSpan));
    expect(TraceService.listGraphTraces).toHaveBeenCalledTimes(2);
  });

  it('reports an evicted selected run locally without replacing the graph list', async () => {
    const graphRecords = [trace('1', '31')];
    vi.mocked(TraceService.listGraphTraces).mockResolvedValueOnce(graphRecords);
    vi.mocked(TraceService.getRunTrace).mockRejectedValueOnce({
      code: 'trace_not_found',
      message: 'The requested trace is no longer retained.',
    });
    await renderHook();

    await act(async () => {
      await current?.selectRun('31');
    });

    expect(current?.graphTraces).toEqual(graphRecords.map(projectTraceSpan));
    expect(current).toMatchObject({
      selectedRunId: '31',
      runTrace: [],
      runLoading: false,
      runError: {
        code: 'trace_not_found',
        message: 'The requested trace is no longer retained.',
      },
      selectedRunNotFound: true,
    });
  });
});
