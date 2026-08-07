// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { clearWorksheetPreviewCache } from '@/services/worksheet/worksheetPreviewCache';
import { fetchWorksheetPreview } from '@/services/worksheet/worksheetDataService';
import type { WorksheetDocument, WorksheetPreviewPayload } from '@/shared/types/domain';
import { WorksheetChartPreview } from './WorksheetChartPreview';

vi.mock('@/services/worksheet/worksheetDataService', () => ({ fetchWorksheetPreview: vi.fn() }));
vi.mock('@/views/PlotView/Scatter', () => ({
  default: ({ data }: { data: Array<{ x: number; y: number }> }) => <div>{`scatter:${data[0]?.x}`}</div>,
}));
vi.mock('@/views/PlotView/Line', () => ({ default: () => <div>line</div> }));
vi.mock('@/views/PlotView/Histogram', () => ({ default: () => <div>histogram</div> }));
vi.mock('./WorksheetEmptyState', () => ({ WorksheetEmptyState: () => <div>empty</div> }));

const projectA = '00000000-0000-0000-0000-000000000601';
const projectB = '00000000-0000-0000-0000-000000000602';
const worksheet: WorksheetDocument = {
  schemaVersion: 3,
  revision: 0,
  id: 'worksheet-1',
  name: 'Worksheet',
  databaseId: 'sales',
  chartType: 'scatter',
  encodings: { x: 'x', y: 'y' },
};
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => { resolve = settle; });
  return { promise, resolve };
}

function scatter(x: number): WorksheetPreviewPayload {
  return {
    kind: 'scatter',
    pair: {
      data: [{ x, y: x }],
      xLabel: 'x',
      yLabel: 'y',
      xFormat: 'number',
      yFormat: 'number',
    },
  };
}

describe('WorksheetChartPreview project cache identity', () => {
  let root: Root;
  let host: HTMLDivElement;

  beforeEach(() => {
    vi.restoreAllMocks();
    vi.useFakeTimers();
    clearWorksheetPreviewCache();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectA, 0);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.useRealTimers();
  });

  it('does not reuse an old in-flight payload after same-ID project replacement', async () => {
    const oldRequest = deferred<WorksheetPreviewPayload>();
    vi.mocked(fetchWorksheetPreview)
      .mockReturnValueOnce(oldRequest.promise)
      .mockResolvedValueOnce(scatter(2));

    act(() => root.render(<WorksheetChartPreview document={worksheet} />));
    await act(async () => vi.advanceTimersByTimeAsync(300));
    expect(fetchWorksheetPreview).toHaveBeenCalledTimes(1);

    act(() => root.unmount());
    host.remove();
    projectPublicationCoordinator.startProject(projectB, 0);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root.render(<WorksheetChartPreview document={worksheet} />));
    await act(async () => vi.advanceTimersByTimeAsync(300));

    expect(fetchWorksheetPreview).toHaveBeenCalledTimes(2);
    expect(host.textContent).toContain('scatter:2');

    await act(async () => {
      oldRequest.resolve(scatter(1));
      await Promise.resolve();
    });
    expect(host.textContent).toContain('scatter:2');
    expect(host.textContent).not.toContain('scatter:1');
  });
});
