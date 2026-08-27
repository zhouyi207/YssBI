// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { ResultViewShell } from '@/features/core/resultSource/components/ResultViewShell';
import type { ResultDescriptor } from '@/shared/types/dto/result';
import { ResultContent } from './ResultContent';

const mocks = vi.hoisted(() => ({
  loadPresentationWindow: vi.fn(),
  parsePlotPayload: vi.fn(),
  plotResultView: vi.fn(),
  launchInspectablePresentation: vi.fn(),
}));

vi.mock('@/features/core/resultSource', async () => {
  const { ResultViewShell } = await vi.importActual<
    typeof import('@/features/core/resultSource/components/ResultViewShell')
  >('@/features/core/resultSource/components/ResultViewShell');

  return {
    ReportResultView: () => null,
    UnifiedResultView: ({ payload }: { payload: ResultDescriptor }) => (
      <ResultViewShell
        title={payload.title}
        meta={<span data-testid="result-meta">result meta</span>}
        toolbar={<button type="button" data-testid="result-toolbar">toolbar</button>}
      >
        <div data-testid="result-body">result body</div>
      </ResultViewShell>
    ),
  };
});

vi.mock('@/features/application/presentation', () => ({
  loadPresentationWindow: mocks.loadPresentationWindow,
  parsePlotPayload: mocks.parsePlotPayload,
  presentationWindowErrorMessage: () => null,
}));

vi.mock('@/features/application/execution/openInspectableResult', () => ({
  launchInspectablePresentation: mocks.launchInspectablePresentation,
}));

vi.mock('@/features/application/presentation/PlotResultView', () => ({
  PlotResultView: (props: unknown) => {
    mocks.plotResultView(props);
    return <div data-testid="plot-preview">plot preview</div>;
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const plotData = { points: [[1, 2]] };
const parsedPlotPayload = {
  kind: 'scatter' as const,
  data: { data: [{ x: 1, y: 2 }] },
};

const descriptor: ResultDescriptor = {
  resultId: 'plot-1',
  state: { kind: 'ready' },
  provenance: {
    runId: 'run-1',
    activationId: 'activation-1',
    graphPath: 'events/Main.yssbi-event',
    graphRevision: '1',
    nodeId: 'plot-node',
    output: {
      graphPath: 'events/Main.yssbi-event',
      port: { kind: 'declared', nodeId: 'plot-node', portKey: 'plot' },
    },
    createdAtMs: '1787270400000',
  },
  presentation: { kind: 'plot', chart: 'scatter' },
  valueKind: 'scalar',
  metadata: null,
  totalCount: null,
  title: 'Scatter',
};

async function flush(): Promise<void> {
  await act(async () => { await Promise.resolve(); });
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe('ResultContent', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeAll(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
  });

  afterAll(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
      .IS_REACT_ACT_ENVIRONMENT = false;
  });

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.loadPresentationWindow.mockResolvedValue({
      status: 'ready',
      descriptor,
      payload: { mode: 'plot', chart: 'scatter', data: plotData },
    });
    mocks.parsePlotPayload.mockReturnValue(parsedPlotPayload);
    mocks.launchInspectablePresentation.mockResolvedValue(undefined);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('renders inspector results without the standalone title hierarchy', async () => {
    const inspectorDescriptor: ResultDescriptor = {
      ...descriptor,
      resultId: 'inspector-1',
      presentation: { kind: 'inspector' },
      title: 'Inspector result',
    };
    mocks.loadPresentationWindow.mockResolvedValueOnce({
      status: 'ready',
      descriptor: inspectorDescriptor,
      payload: { mode: 'inspector', descriptor: inspectorDescriptor },
    });

    act(() => root.render(<ResultContent resultId="inspector-1" />));
    await flush();

    expect(container.querySelector('h1')).toBeNull();
    expect(container.querySelector('[data-testid="result-meta"]')).toBeNull();
    expect(container.querySelector('[data-testid="result-body"]')).not.toBeNull();
    expect(container.querySelector('[data-result-view-toolbar]')).not.toBeNull();
  });

  it('keeps the full title hierarchy when the result shell is standalone', () => {
    act(() => root.render(
      <ResultViewShell
        title="Standalone result"
        meta={<span data-testid="standalone-meta">standalone meta</span>}
      >
        <div>standalone body</div>
      </ResultViewShell>,
    ));

    expect(container.querySelector('h1')?.textContent).toBe('Standalone result');
    expect(container.querySelector('[data-testid="standalone-meta"]')?.textContent)
      .toBe('standalone meta');
  });

  it('renders a plot preview and expands only after the user requests it', async () => {
    act(() => root.render(<ResultContent resultId="plot-1" />));
    await flush();

    expect(container.querySelector('[data-testid="plot-preview"]')).not.toBeNull();
    expect(mocks.parsePlotPayload).toHaveBeenCalledOnce();
    expect(mocks.parsePlotPayload.mock.calls[0][0]).toBe('scatter');
    expect(mocks.parsePlotPayload.mock.calls[0][1]).toBe(plotData);
    expect(mocks.plotResultView).toHaveBeenCalled();
    const renderedPlotProps = mocks.plotResultView.mock.calls[
      mocks.plotResultView.mock.calls.length - 1
    ][0];
    expect(renderedPlotProps.payload).toBe(parsedPlotPayload);
    expect(mocks.launchInspectablePresentation).not.toHaveBeenCalled();
    const expand = [...container.querySelectorAll('button')]
      .find((button) => button.textContent === 'detail.result.expandPlot');
    expect(expand).toBeDefined();

    act(() => expand?.click());
    await flush();
    expect(mocks.launchInspectablePresentation).toHaveBeenCalledWith(
      descriptor,
      'detail.result.expandPlot',
    );
  });

  it('ignores an expansion failure from a previously rendered result', async () => {
    const expansion = deferred<void>();
    const nextDescriptor: ResultDescriptor = {
      ...descriptor,
      resultId: 'plot-2',
      title: 'Next plot',
    };
    mocks.loadPresentationWindow
      .mockResolvedValueOnce({
        status: 'ready',
        descriptor,
        payload: { mode: 'plot', chart: 'scatter', data: { points: [[1, 2]] } },
      })
      .mockResolvedValueOnce({
        status: 'ready',
        descriptor: nextDescriptor,
        payload: { mode: 'plot', chart: 'scatter', data: { points: [[3, 4]] } },
      });
    mocks.launchInspectablePresentation.mockReturnValueOnce(expansion.promise);

    act(() => root.render(<ResultContent resultId="plot-1" />));
    await flush();
    const expand = [...container.querySelectorAll('button')]
      .find((button) => button.textContent === 'detail.result.expandPlot');
    act(() => expand?.click());

    act(() => root.render(<ResultContent resultId="plot-2" />));
    await flush();
    await act(async () => {
      expansion.reject(new Error('stale window failure'));
      await Promise.resolve();
    });

    expect(container.querySelector('[data-testid="plot-preview"]')).not.toBeNull();
    expect(container.querySelector('[role="alert"]')).toBeNull();
  });

  it('keeps the preview mounted and shows an Alert when expansion fails', async () => {
    mocks.launchInspectablePresentation.mockRejectedValueOnce(new Error('window failed'));
    act(() => root.render(<ResultContent resultId="plot-1" />));
    await flush();

    const expand = [...container.querySelectorAll('button')]
      .find((button) => button.textContent === 'detail.result.expandPlot');
    act(() => expand?.click());
    await flush();

    expect(container.querySelector('[data-testid="plot-preview"]')).not.toBeNull();
    expect(container.querySelector('[role="alert"]')?.textContent)
      .toContain('detail.result.expandFailed');
  });
});
