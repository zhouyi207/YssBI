// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { GraphTraceDetailsState } from '@/features/application/observability/useGraphTraceDetails';
import type { TraceRecordDto } from '@/shared/types/dto/trace';
import { GraphTraceDetails } from './GraphTraceDetails';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const hookState = vi.hoisted(() => ({
  current: null as GraphTraceDetailsState | null,
}));

vi.mock('@/features/application/observability/useGraphTraceDetails', () => ({
  useGraphTraceDetails: () => hookState.current,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => ({
      'detail.trace.title': 'Developer trace',
      'detail.trace.refresh': 'Refresh',
      'detail.trace.loading': 'Loading trace…',
      'detail.trace.empty': 'No retained traces',
      'detail.trace.graphError': 'Unable to load graph traces',
      'detail.trace.runError': 'Unable to load run trace',
      'detail.trace.runNotFound': 'Run trace is no longer retained',
      'detail.trace.runs': 'Runs',
      'detail.trace.allRuns': 'All graph traces',
      'detail.trace.run': 'Run',
      'detail.trace.sequence': 'Sequence',
      'detail.trace.kind': 'Kind',
      'detail.trace.status': 'Status',
      'detail.trace.correlation': 'Correlation',
      'detail.trace.projectSession': 'Project session',
      'detail.trace.graphPath': 'Graph path',
      'detail.trace.graphRevision': 'Graph revision',
      'detail.trace.registryFingerprint': 'Registry fingerprint',
      'detail.trace.resourceVersions': 'Resource versions',
      'detail.trace.compileId': 'Compile ID',
      'detail.trace.runId': 'Run ID',
      'detail.trace.nodeId': 'Node ID',
      'detail.trace.nodeTypeId': 'Node type ID',
      'detail.trace.parentCall': 'Parent call',
      'detail.trace.publicFields': 'Public fields',
      'detail.trace.redacted': '[redacted]',
      'detail.trace.none': '—',
    } as Record<string, string>)[key] ?? key,
  }),
}));

const graphPath = 'events/Main.yssbi-event';

function trace(overrides: Partial<TraceRecordDto> = {}): TraceRecordDto {
  return {
    sequence: '7',
    kind: 'operation',
    status: 'succeeded',
    correlation: {
      projectSessionId: 'session-public',
      graphPath,
      graphRevision: '3',
      registryFingerprint: 'registry-public',
      resourceVersions: { inventory: 'version-public' },
      compileId: '5',
      selectionDigest: 'demand-selection-a',
      runId: '41',
      nodeId: 'node-public',
      nodeTypeId: 'functions/public-node',
      parentCall: '2',
    },
    fields: {
      backend: { type: 'text', value: 'polars' },
      attempt: { type: 'integer', value: 1 },
      credential: { type: 'redacted' },
    },
    ...overrides,
  };
}

function readyState(overrides: Partial<GraphTraceDetailsState> = {}): GraphTraceDetailsState {
  return {
    graphTraces: [trace()],
    graphLoading: false,
    graphError: null,
    selectedRunId: null,
    runTrace: [],
    runLoading: false,
    runError: null,
    selectedRunNotFound: false,
    refresh: vi.fn(async () => undefined),
    selectRun: vi.fn(async () => undefined),
    ...overrides,
  };
}

describe('GraphTraceDetails', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    hookState.current = readyState();
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  function renderDetails() {
    act(() => root.render(<GraphTraceDetails graphPath={graphPath} />));
  }

  function clickButton(label: string) {
    const button = Array.from(host.querySelectorAll('button')).find((candidate) =>
      candidate.textContent?.includes(label),
    );
    expect(button, `button containing ${label}`).toBeTruthy();
    act(() => button!.dispatchEvent(new MouseEvent('click', { bubbles: true })));
  }

  function expand() {
    clickButton('Developer trace');
  }

  it('keeps developer traces collapsed by default', () => {
    renderDetails();

    expect(host.textContent).toContain('Developer trace');
    expect(host.textContent).not.toContain('Sequence');
    expect(host.querySelector('[aria-expanded="false"]')).not.toBeNull();
  });

  it('refreshes through the read-only hook action', () => {
    hookState.current = readyState({
      refresh: async () => {
        hookState.current = readyState({ graphLoading: true });
        renderDetails();
      },
    });
    renderDetails();
    expand();

    clickButton('Refresh');

    expect(host.textContent).toContain('Loading trace…');
  });

  it('renders sequence, status, and the complete allowlisted correlation', () => {
    renderDetails();
    expand();

    expect(host.textContent).toContain('7');
    expect(host.textContent).toContain('operation');
    expect(host.textContent).toContain('succeeded');
    expect(host.textContent).toContain('session-public');
    expect(host.textContent).toContain(graphPath);
    expect(host.textContent).toContain('registry-public');
    expect(host.textContent).toContain('inventory');
    expect(host.textContent).toContain('version-public');
    expect(host.textContent).toContain('functions/public-node');
  });

  it('selects a retained run and replaces the graph projection with its trace', () => {
    hookState.current = readyState({
      selectRun: async (runId) => {
        hookState.current = readyState({
          selectedRunId: runId,
          runTrace: [trace({ sequence: '99', status: 'failed' })],
        });
        renderDetails();
      },
    });
    renderDetails();
    expand();

    clickButton('Run 41');

    expect(host.textContent).toContain('99');
    expect(host.textContent).toContain('failed');
  });

  it('renders public fields and an explicit redacted marker without leaking extra values', () => {
    const unsafeRecord = {
      ...trace(),
      privateRuntimeValue: 'must-never-render',
    } as TraceRecordDto;
    hookState.current = readyState({ graphTraces: [unsafeRecord] });
    renderDetails();
    expand();

    expect(host.textContent).toContain('backend');
    expect(host.textContent).toContain('polars');
    expect(host.textContent).toContain('attempt');
    expect(host.textContent).toContain('[redacted]');
    expect(host.textContent).not.toContain('must-never-render');
  });

  it.each([
    [readyState({ graphLoading: true }), 'Loading trace…'],
    [readyState({ graphTraces: [], graphError: { code: 'trace_query_failed', message: 'secret backend detail' } }), 'Unable to load graph traces'],
    [readyState({ selectedRunId: '41', runError: { code: 'trace_query_failed', message: 'secret run detail' } }), 'Unable to load run trace'],
    [readyState({ selectedRunId: '41', selectedRunNotFound: true, runError: { code: 'trace_not_found', message: 'evicted' } }), 'Run trace is no longer retained'],
    [readyState({ graphTraces: [] }), 'No retained traces'],
  ])('renders privacy-safe loading, error, not-found, and empty states', (state, message) => {
    hookState.current = state;
    renderDetails();
    expand();

    expect(host.textContent).toContain(message);
    expect(host.textContent).not.toContain('secret backend detail');
    expect(host.textContent).not.toContain('secret run detail');
    expect(host.textContent).not.toContain('evicted');
  });

  it('does not expose mutation, export, or retry controls', () => {
    renderDetails();
    expand();

    const labels = Array.from(host.querySelectorAll('button')).map((button) => button.textContent);
    expect(labels.some((label) => /clear|delete|export|retry/i.test(label ?? ''))).toBe(false);
    expect(labels.some((label) => label?.includes('Refresh'))).toBe(true);
  });
});
