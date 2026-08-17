import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import traceBundleWire from '@/tests/fixtures/node-system-contracts/trace-bundle-wire.json';
import {
  parseRunTraceBundle,
  parseTraceBundleList,
  type CompilationTraceBundleDto,
  type RunTraceBundleDto,
  type TraceSpanDto,
} from '@/shared/types/dto/trace';
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

const metadata = {
  provenanceScopes: [{
    projectSessionId: 'project-session-1',
    graphPath: 'events/Main.yssbi-event',
    graphRevision: '7' as const,
    registryFingerprint: 'registry-fingerprint-1',
    resourceVersions: { dataset: 'version-1' },
    compileId: '9007199254740994' as const,
  }],
  truncated: false,
  droppedSpanCount: '0' as const,
  estimatedBytes: '512' as const,
};

const runBundle: RunTraceBundleDto = {
  bundleKind: 'run',
  runId: '9007199254740995',
  compileId: '9007199254740994',
  graphPath: 'events/Main.yssbi-event',
  selectionDigest: 'demand-selection-a',
  incidentId: 'incident-public-id',
  metadata,
  spans: [traceSpan],
};

const compilationBundle: CompilationTraceBundleDto = {
  bundleKind: 'compilation',
  compileId: '9007199254740994',
  graphPath: 'events/Main.yssbi-event',
  metadata,
  spans: [{
    ...traceSpan,
    runId: null,
    kind: 'snapshot',
    correlation: { ...traceSpan.correlation, runId: null },
  }],
};

describe('TraceService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('strictly parses the Rust-generated run bundle golden', () => {
    const parsed = parseRunTraceBundle(traceBundleWire);
    expect(parsed).toMatchObject({
      bundleKind: 'run',
      runId: '9007199254740997',
      incidentId: 'contract-incident',
      metadata: {
        truncated: false,
        droppedSpanCount: '0',
        estimatedBytes: '9007199254740999',
      },
    });
    expect(parsed.spans[1]).toMatchObject({
      spanId: '9007199254740994',
      parentSpanId: '9007199254740993',
      outcome: { cleanup: { errorCount: '9007199254740997', panicking: true } },
    });
  });

  it('lists parsed bundles with the exact project and graph arguments', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([compilationBundle, runBundle]);

    await expect(
      TraceService.listGraphTraceBundles('project-instance-1', 'events/Main.yssbi-event'),
    ).resolves.toEqual([compilationBundle, runBundle]);

    expect(invoke).toHaveBeenCalledWith('list_graph_trace_bundles', {
      projectInstanceId: 'project-instance-1',
      graphPath: 'events/Main.yssbi-event',
    });
  });

  it('gets a parsed run bundle with an opaque decimal-string run ID', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(runBundle);

    await expect(
      TraceService.getRunTraceBundle('project-instance-1', '9007199254740995'),
    ).resolves.toEqual(runBundle);

    expect(invoke).toHaveBeenCalledWith('get_run_trace_bundle', {
      projectInstanceId: 'project-instance-1',
      runId: '9007199254740995',
    });
  });

  it('limits frontend validation to the wire contract', () => {
    const backendAuthoritative = {
      ...runBundle,
      spans: [{
        ...traceSpan,
        parentSpanId: traceSpan.spanId,
        startedAt: '100',
        finishedAt: '10',
        correlation: { ...traceSpan.correlation, runId: null },
      }],
    };

    expect(parseRunTraceBundle(backendAuthoritative)).toEqual(backendAuthoritative);
  });

  it('rejects a compilation bundle from the run-only parser', () => {
    expect(() => parseRunTraceBundle(compilationBundle))
      .toThrow('Invalid trace bundle response');
  });

  it.each([
    ['unsafe numeric run ID', { ...runBundle, runId: 9007199254740995 }],
    ['missing bundle metadata', Object.fromEntries(Object.entries(runBundle).filter(([key]) => key !== 'metadata'))],
    ['unknown bundle field', { ...runBundle, status: 'completed' }],
    ['invalid dropped span count', { ...runBundle, metadata: { ...metadata, droppedSpanCount: '-1' } }],
    ['obsolete compilation incident ID', { ...compilationBundle, incidentId: null }],
    ['obsolete span array response', [traceSpan]],
  ])('rejects malformed bundle wire: %s', (_label, wire) => {
    expect(() => parseTraceBundleList(Array.isArray(wire) ? wire : [wire]))
      .toThrow('Invalid trace bundle response');
  });
});
