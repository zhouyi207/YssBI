// @vitest-environment happy-dom
import { act, StrictMode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  RunTraceBundleDto,
  TraceBundleDto,
  TraceDecimalString,
  TraceSpanDto,
} from '@/shared/types/dto/trace';
import { TraceService } from '@/services/nodeSystem/traceService';
import { normalizeIpcError } from '@/services/ipc';
import { projectTraceBundle, useGraphTraceDetails } from './useGraphTraceDetails';

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
    listGraphTraceBundles: vi.fn(),
    getRunTraceBundle: vi.fn(),
  },
}));

function backendError(code: string) {
  return normalizeIpcError('get_run_trace_bundle', { code, details: null, incidentId: null });
}

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

function runBundle(
  runId: TraceDecimalString,
  spanId: TraceDecimalString = '1',
  overrides: Partial<RunTraceBundleDto> = {},
): RunTraceBundleDto {
  const graphPath = 'events/Main.yssbi-event';
  return {
    bundleKind: 'run',
    runId,
    compileId: '10',
    graphPath,
    selectionDigest: 'demand-selection-a',
    incidentId: null,
    metadata: {
      provenanceScopes: [{
        projectSessionId: 'project-session-1',
        graphPath,
        graphRevision: '7',
        registryFingerprint: 'registry-fingerprint-1',
        resourceVersions: { dataset: 'version-1' },
        compileId: '10',
      }],
      truncated: false,
      droppedSpanCount: '0',
      estimatedBytes: '512',
    },
    spans: [trace(spanId, runId)],
    ...overrides,
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
    vi.mocked(TraceService.listGraphTraceBundles).mockReset();
    vi.mocked(TraceService.getRunTraceBundle).mockReset();
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

  it('queries bundles for the initial graph with the captured project identity', async () => {
    const records = [runBundle('11')];
    vi.mocked(TraceService.listGraphTraceBundles).mockResolvedValueOnce(records);

    await renderHook();

    expect(TraceService.listGraphTraceBundles).toHaveBeenCalledWith(
      'project-1',
      'events/Main.yssbi-event',
    );
    expect(current).toMatchObject({
      graphBundles: records,
      graphLoading: false,
      graphError: null,
    });
  });

  it('publishes the current initial query under React Strict Mode', async () => {
    const records = [runBundle('11')];
    vi.mocked(TraceService.listGraphTraceBundles).mockResolvedValue(records);

    await act(async () => {
      root.render(
        <StrictMode>
          <Harness graphPath="events/Main.yssbi-event" />
        </StrictMode>,
      );
      await Promise.resolve();
    });

    expect(current).toMatchObject({
      graphBundles: records,
      graphLoading: false,
      graphError: null,
    });
  });

  it('refreshes the graph list and discards an older completion', async () => {
    const initial = [runBundle('11')];
    const older = deferred<TraceBundleDto[]>();
    const newest = [runBundle('13', '3')];
    vi.mocked(TraceService.listGraphTraceBundles)
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
    expect(current?.graphBundles).toEqual(newest.map(projectTraceBundle));

    await act(async () => older.resolve([runBundle('12', '2')]));
    expect(current?.graphBundles).toEqual(newest.map(projectTraceBundle));
  });

  it('projects bundle metadata and bigint durations without changing opaque IDs', async () => {
    const bundle = runBundle('11', '9007199254740993', {
      incidentId: 'incident-public-id',
      metadata: {
        ...runBundle('11').metadata,
        truncated: true,
        droppedSpanCount: '4',
      },
      spans: [trace(
        '9007199254740993',
        '11',
        undefined,
        '90071992547409930',
        '90071992547410055',
      )],
    });
    vi.mocked(TraceService.listGraphTraceBundles).mockResolvedValueOnce([bundle]);

    await renderHook();

    expect(current?.graphBundles[0]).toMatchObject({
      runId: '11',
      incidentId: 'incident-public-id',
      metadata: { truncated: true, droppedSpanCount: '4' },
      spans: [{
        spanId: '9007199254740993',
        durationNanos: 125n,
      }],
    });
  });

  it('loads a selected run bundle and clears it with the selection', async () => {
    vi.mocked(TraceService.listGraphTraceBundles).mockResolvedValueOnce([runBundle('11')]);
    const selected = runBundle('9007199254740993', '4');
    vi.mocked(TraceService.getRunTraceBundle).mockResolvedValueOnce(selected);
    await renderHook();

    await act(async () => {
      await current?.selectRun('9007199254740993');
    });

    expect(TraceService.getRunTraceBundle).toHaveBeenCalledWith(
      'project-1',
      '9007199254740993',
    );
    expect(current).toMatchObject({
      selectedRunId: '9007199254740993',
      runBundle: selected,
      runLoading: false,
      runError: null,
    });

    await act(async () => {
      await current?.selectRun(null);
    });
    expect(current).toMatchObject({
      selectedRunId: null,
      runBundle: null,
      runLoading: false,
      runError: null,
    });
  });

  it('suppresses stale-project completions from graph and run queries', async () => {
    const graphRequest = deferred<TraceBundleDto[]>();
    const runRequest = deferred<RunTraceBundleDto>();
    vi.mocked(TraceService.listGraphTraceBundles)
      .mockReturnValueOnce(graphRequest.promise)
      .mockResolvedValueOnce([]);
    vi.mocked(TraceService.getRunTraceBundle).mockReturnValueOnce(runRequest.promise);
    await renderHook();

    act(() => {
      void current?.selectRun('21');
    });
    projectIdentity.activeId = 'project-2';
    await act(async () => {
      graphRequest.resolve([runBundle('21')]);
      runRequest.resolve(runBundle('21', '2'));
      await Promise.all([graphRequest.promise, runRequest.promise]);
    });

    expect(current?.graphBundles).toEqual([]);
    expect(current?.runBundle).toBeNull();
    expect(current?.graphError).toBeNull();
    expect(current?.runError).toBeNull();
    expect(current?.graphLoading).toBe(false);
    expect(current?.runLoading).toBe(false);
  });

  it('re-arms the graph query when project identity changes without remounting', async () => {
    const staleRequest = deferred<TraceBundleDto[]>();
    const currentRecords = [runBundle('22', '2')];
    vi.mocked(TraceService.listGraphTraceBundles)
      .mockReturnValueOnce(staleRequest.promise)
      .mockResolvedValueOnce(currentRecords);
    await renderHook();

    projectIdentity.activeId = 'project-2';
    await act(async () => {
      root.render(<Harness graphPath="events/Main.yssbi-event" />);
      await Promise.resolve();
    });

    expect(TraceService.listGraphTraceBundles).toHaveBeenCalledTimes(2);
    expect(current).toMatchObject({
      graphBundles: currentRecords,
      graphLoading: false,
      graphError: null,
    });

    await act(async () => staleRequest.resolve([runBundle('11')]));
    expect(current?.graphBundles).toEqual(currentRecords.map(projectTraceBundle));
  });

  it('reports an evicted selected run locally without replacing the graph list', async () => {
    const graphRecords = [runBundle('31')];
    vi.mocked(TraceService.listGraphTraceBundles).mockResolvedValueOnce(graphRecords);
    vi.mocked(TraceService.getRunTraceBundle)
      .mockRejectedValueOnce(backendError('trace_not_found'));
    await renderHook();

    await act(async () => {
      await current?.selectRun('31');
    });

    expect(current?.graphBundles).toEqual(graphRecords.map(projectTraceBundle));
    expect(current).toMatchObject({
      selectedRunId: '31',
      runBundle: null,
      runLoading: false,
      runError: {
        code: 'trace_not_found',
        message: 'Unable to load trace details.',
      },
      selectedRunNotFound: true,
    });
  });
});
