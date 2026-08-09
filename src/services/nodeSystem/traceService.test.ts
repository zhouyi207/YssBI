import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import traceSpanWire from '@/tests/fixtures/node-system-contracts/trace-span-wire.json';
import { parseTraceSpanList, type TraceSpanDto } from '@/shared/types/dto/trace';
import { TraceService } from './traceService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const traceSpan: TraceSpanDto = {
  spanId: '9007199254740993',
  parentSpanId: null,
  runId: '9007199254740995',
  operationId: null,
  activationId: null,
  attemptId: null,
  kind: 'run',
  startedAt: '9007199254740997',
  finishedAt: '9007199254741007',
  outcome: 'success',
  correlation: {
    projectSessionId: 'project-session-1',
    graphPath: 'events/Main.yssbi-event',
    graphRevision: '7',
    registryFingerprint: 'registry-fingerprint-1',
    resourceVersions: { dataset: 'version-1' },
    compileId: '9007199254740994',
    selectionDigest: 'demand-selection-a',
    runId: '9007199254740995',
    nodeId: null,
    nodeTypeId: null,
    parentCall: null,
  },
};

function runtimeHierarchy(): TraceSpanDto[] {
  const run: TraceSpanDto = {
    ...traceSpan,
    spanId: '601',
    startedAt: '10',
    finishedAt: '100',
  };
  const attempt: TraceSpanDto = {
    ...run,
    spanId: '602',
    parentSpanId: run.spanId,
    operationId: 'operation-a',
    activationId: '1',
    attemptId: '1',
    kind: 'operationAttempt',
    startedAt: '20',
    finishedAt: '80',
    outcome: 'retry',
  };
  const adapter: TraceSpanDto = {
    ...attempt,
    spanId: '603',
    parentSpanId: attempt.spanId,
    kind: 'adapterIo',
    startedAt: '30',
    finishedAt: '70',
    outcome: 'internalAborted',
  };
  const cleanup: TraceSpanDto = {
    ...run,
    spanId: '604',
    parentSpanId: run.spanId,
    kind: 'cleanup',
    startedAt: '90',
    finishedAt: '95',
    outcome: { cleanup: { errorCount: '0', panicking: false } },
  };
  return [run, attempt, adapter, cleanup];
}

describe('TraceService', () => {
  it('strictly parses the Rust-generated completed-span golden', () => {
    const parsed = parseTraceSpanList(traceSpanWire);
    expect(parsed).toHaveLength(2);
    expect(parsed[1]).toMatchObject({
      spanId: '9007199254740994',
      parentSpanId: '9007199254740993',
      startedAt: '9007199254740995',
      finishedAt: '9007199254740996',
      outcome: { cleanup: { errorCount: '9007199254740997', panicking: true } },
    });
  });
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue([traceSpan]);
  });

  it('lists parsed completed spans with the exact project and graph arguments', async () => {
    await expect(
      TraceService.listGraphTraces('project-instance-1', 'events/Main.yssbi-event'),
    ).resolves.toEqual([traceSpan]);

    expect(invoke).toHaveBeenCalledWith('list_graph_traces', {
      projectInstanceId: 'project-instance-1',
      graphPath: 'events/Main.yssbi-event',
    });
  });

  it('gets a parsed run trace with an opaque decimal-string run ID', async () => {
    await expect(
      TraceService.getRunTrace('project-instance-1', '9007199254740995'),
    ).resolves.toEqual([traceSpan]);

    expect(invoke).toHaveBeenCalledWith('get_run_trace', {
      projectInstanceId: 'project-instance-1',
      runId: '9007199254740995',
    });
  });

  it('accepts a valid null-run compiler hierarchy', () => {
    const snapshot: TraceSpanDto = {
      ...traceSpan,
      spanId: '101',
      runId: null,
      kind: 'snapshot',
      correlation: { ...traceSpan.correlation, runId: null },
    };
    const analysis: TraceSpanDto = {
      ...snapshot,
      spanId: '102',
      parentSpanId: snapshot.spanId,
      kind: 'analysis',
    };

    expect(parseTraceSpanList([snapshot, analysis])).toEqual([snapshot, analysis]);
  });

  it('parses a retained unexpected cleanup unwind fallback', () => {
    const spans = runtimeHierarchy().map((span) => (
      span.kind === 'cleanup'
        ? { ...span, outcome: 'internalAborted' as const }
        : span
    ));
    expect(parseTraceSpanList(spans)).toEqual(spans);
  });

  it('accepts equal monotonic endpoints', () => {
    const spans = runtimeHierarchy().map((span) => (
      span.kind === 'cleanup'
        ? { ...span, startedAt: '90' as const, finishedAt: '90' as const }
        : span
    ));
    expect(parseTraceSpanList(spans)).toEqual(spans);
  });

  it('accepts valid runtime retry, panic-drop, and cleanup semantics', () => {
    const spans = runtimeHierarchy();
    expect(parseTraceSpanList(spans)).toEqual(spans);
  });

  it.each([
    ['compiler span with runtime run identity', () => [{ ...traceSpan, kind: 'snapshot' as const }]],
    ['runtime run without run identity', () => [{
      ...traceSpan,
      runId: null,
      correlation: { ...traceSpan.correlation, runId: null },
    }]],
    ['attempt without complete operation identity', () => runtimeHierarchy().map((span) => (
      span.kind === 'operationAttempt' ? { ...span, attemptId: null } : span
    ))],
    ['notReached on run', () => runtimeHierarchy().map((span) => (
      span.kind === 'run' ? { ...span, outcome: 'notReached' as const } : span
    ))],
    ['retry on adapter', () => runtimeHierarchy().map((span) => (
      span.kind === 'adapterIo' ? { ...span, outcome: 'retry' as const } : span
    ))],
    ['cleanup metadata on attempt', () => runtimeHierarchy().map((span) => (
      span.kind === 'operationAttempt'
        ? { ...span, outcome: { cleanup: { errorCount: '0' as const, panicking: false } } }
        : span
    ))],
    ['reached cleanup with generic success', () => runtimeHierarchy().map((span) => (
      span.kind === 'cleanup' ? { ...span, outcome: 'success' as const } : span
    ))],
    ['reached cleanup with generic error', () => runtimeHierarchy().map((span) => (
      span.kind === 'cleanup' ? { ...span, outcome: 'error' as const } : span
    ))],
    ['reversed compiler endpoints', () => [{
      ...traceSpan,
      spanId: '590',
      runId: null,
      kind: 'snapshot' as const,
      startedAt: '50' as const,
      finishedAt: '40' as const,
      correlation: { ...traceSpan.correlation, runId: null },
    }]],
    ['reversed runtime child endpoints within parent bounds', () => runtimeHierarchy().map((span) => (
      span.kind === 'operationAttempt'
        ? { ...span, startedAt: '60' as const, finishedAt: '40' as const }
        : span
    ))],
    ['runtime child starts before parent', () => runtimeHierarchy().map((span) => (
      span.kind === 'operationAttempt' ? { ...span, startedAt: '9' as const } : span
    ))],
    ['runtime child finishes after parent', () => runtimeHierarchy().map((span) => (
      span.kind === 'operationAttempt' ? { ...span, finishedAt: '101' as const } : span
    ))],
    ['adapter interval escapes attempt', () => runtimeHierarchy().map((span) => (
      span.kind === 'adapterIo' ? { ...span, finishedAt: '81' as const } : span
    ))],
  ])('rejects invalid kind/outcome/run/time semantics: %s', (_label, build) => {
    expect(() => parseTraceSpanList(build())).toThrow('Invalid trace span response');
  });

  it.each([
    ['self-parent', () => [{ ...traceSpan, parentSpanId: traceSpan.spanId }]],
    ['two-span cycle', () => [
      { ...traceSpan, spanId: '201', parentSpanId: '202' },
      { ...traceSpan, spanId: '202', parentSpanId: '201' },
    ]],
    ['deeper cycle', () => [
      { ...traceSpan, spanId: '301', parentSpanId: '303' },
      { ...traceSpan, spanId: '302', parentSpanId: '301' },
      { ...traceSpan, spanId: '303', parentSpanId: '302' },
    ]],
    ['cross-run parent', () => [
      { ...traceSpan, spanId: '401', runId: '41', correlation: { ...traceSpan.correlation, runId: '41' } },
      { ...traceSpan, spanId: '402', parentSpanId: '401', runId: '42', correlation: { ...traceSpan.correlation, runId: '42' } },
    ]],
    ['cross-project parent', () => [
      { ...traceSpan, spanId: '451' },
      { ...traceSpan, spanId: '452', parentSpanId: '451', correlation: { ...traceSpan.correlation, projectSessionId: 'project-session-2' } },
    ]],
    ['cross-graph parent', () => [
      { ...traceSpan, spanId: '501' },
      { ...traceSpan, spanId: '502', parentSpanId: '501', correlation: { ...traceSpan.correlation, graphPath: 'events/Other' } },
    ]],
  ])('rejects incompatible hierarchy: %s', (_label, build) => {
    expect(() => parseTraceSpanList(build())).toThrow('Invalid trace span response');
  });

  it('validates a large linear hierarchy without recursive traversal', () => {
    const spans = Array.from({ length: 10_000 }, (_, index): TraceSpanDto => ({
      ...traceSpan,
      spanId: String(10_000 + index) as TraceSpanDto['spanId'],
      parentSpanId: index === 0 ? null : String(10_000 + index - 1) as TraceSpanDto['spanId'],
    }));

    expect(parseTraceSpanList(spans)).toHaveLength(10_000);
  });

  it.each([
    ['unsafe numeric span ID', { ...traceSpan, spanId: 9007199254740993 }],
    ['missing nullable parent', Object.fromEntries(Object.entries(traceSpan).filter(([key]) => key !== 'parentSpanId'))],
    ['unknown field', { ...traceSpan, status: 'succeeded' }],
    ['negative timestamp', { ...traceSpan, startedAt: '-1' }],
    ['finish before start', { ...traceSpan, finishedAt: '1' }],
    ['legacy status record', {
      sequence: '1',
      kind: 'run',
      status: 'started',
      correlation: traceSpan.correlation,
      fields: {},
    }],
    ['legacy start/finish pair', [
      { spanId: '1', phase: 'start', timestamp: '2' },
      { spanId: '1', phase: 'finish', timestamp: '3' },
    ]],
  ])('rejects %s before returning data', async (_label, wire) => {
    vi.mocked(invoke).mockResolvedValueOnce(Array.isArray(wire) ? wire : [wire]);

    await expect(
      TraceService.listGraphTraces('project-instance-1', 'events/Main.yssbi-event'),
    ).rejects.toThrow('Invalid trace span response');
  });
});
