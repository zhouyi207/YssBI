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
vi.mock('@/views/PlotView/Scatter', () => ({ default: () => <div>scatter plot</div> }));
vi.mock('@/views/PlotView/Line', () => ({ default: () => <div>line plot</div> }));
vi.mock('@/views/PlotView/Histogram', () => ({ default: () => <div>histogram plot</div> }));
vi.mock('./WorksheetEmptyState', () => ({ WorksheetEmptyState: () => <div>empty</div> }));

const projectId = '00000000-0000-0000-0000-000000000701';
const worksheet: WorksheetDocument = {
  schemaVersion: 3,
  revision: 0,
  databaseId: 'sales',
  chartType: 'histogram',
  encodings: { x: 'amount' },
};
const chartCases: Array<{
  kind: 'histogram' | 'line' | 'scatter';
  marker: string;
  payload: WorksheetPreviewPayload;
}> = [
  {
    kind: 'histogram',
    marker: 'histogram plot',
    payload: {
      kind: 'histogram',
      bins: [{ label: '10', count: 2 }],
      xLabel: 'amount',
      yLabel: 'count',
    },
  },
  {
    kind: 'line',
    marker: 'line plot',
    payload: {
      kind: 'line',
      pair: {
        data: [{ x: 1, y: 2 }],
        xLabel: 'time',
        yLabel: 'amount',
        xFormat: 'number',
        yFormat: 'number',
      },
    },
  },
  {
    kind: 'scatter',
    marker: 'scatter plot',
    payload: {
      kind: 'scatter',
      pair: {
        data: [{ x: 1, y: 2 }],
        xLabel: 'x',
        yLabel: 'y',
        xFormat: 'number',
        yFormat: 'number',
      },
    },
  },
];
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('WorksheetChartPreview selection boundary', () => {
  let root: Root;
  let host: HTMLDivElement;

  beforeEach(() => {
    vi.restoreAllMocks();
    vi.useFakeTimers();
    clearWorksheetPreviewCache();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectId, 0);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.useRealTimers();
  });

  it.each(chartCases)(
    'isolates $kind output in the same non-selectable chart region',
    async ({ marker, payload }) => {
      vi.mocked(fetchWorksheetPreview).mockResolvedValue(payload);

      act(() => root.render(
        <WorksheetChartPreview worksheetPath="worksheets/Worksheet.yssbi-worksheet" document={worksheet} />,
      ));
      await act(async () => vi.advanceTimersByTimeAsync(300));

      const chartRegion = host.querySelector('[data-worksheet-chart-region]');
      expect(chartRegion).not.toBeNull();
      expect(chartRegion?.classList.contains('select-none')).toBe(true);
      expect(chartRegion?.textContent).toContain(marker);
    },
  );

  it('keeps an error result outside the chart region and selectable', async () => {
    vi.mocked(fetchWorksheetPreview).mockResolvedValue({ kind: 'error', message: 'preview failed' });

    act(() => root.render(
      <WorksheetChartPreview worksheetPath="worksheets/Worksheet.yssbi-worksheet" document={worksheet} />,
    ));
    await act(async () => vi.advanceTimersByTimeAsync(300));

    const errorElement = Array.from(host.querySelectorAll('div')).find(
      (element) => element.childElementCount === 0 && element.textContent === 'preview failed',
    );
    expect(errorElement).toBeDefined();
    expect(errorElement?.closest('[data-worksheet-chart-region]')).toBeNull();
    expect(errorElement?.classList.contains('select-none')).toBe(false);
  });

  it('keeps a fetched empty preview outside any chart region', async () => {
    vi.mocked(fetchWorksheetPreview).mockResolvedValue({ kind: 'empty' });

    act(() => root.render(
      <WorksheetChartPreview worksheetPath="worksheets/Worksheet.yssbi-worksheet" document={worksheet} />,
    ));
    await act(async () => vi.advanceTimersByTimeAsync(300));

    expect(host.textContent).toContain('empty');
    expect(host.querySelector('[data-worksheet-chart-region]')).toBeNull();
  });
});
