// @vitest-environment happy-dom

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ParsedPlotPayload } from '@/shared/types/dto/plotPayload';
import { PlotResultView } from './PlotResultView';

const calls = vi.hoisted(() => ({
  lineControls: vi.fn(),
  scatter: vi.fn(),
  correlogram: vi.fn(),
}));

vi.mock('./LinePlotControls', () => ({
  LinePlotControls: (props: unknown) => {
    calls.lineControls(props);
    return <div data-result-renderer="line-controls" />;
  },
}));
vi.mock('@/shared/charts/cartesian/ScatterChart', () => ({
  ScatterChart: (props: unknown) => {
    calls.scatter(props);
    return <div data-result-renderer="scatter" />;
  },
}));
vi.mock('@/shared/charts/statistical/CorrelogramChart', () => {
  const CorrelogramChart = (props: unknown) => {
    calls.correlogram(props);
    return <div data-result-renderer="correlogram" />;
  };
  return { CorrelogramChart, default: CorrelogramChart };
});
vi.mock('@/shared/charts/core/theme', () => ({
  useChartTheme: () => ({ series: { secondary: 'result-secondary' } }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let root: Root;

function renderResult(payload: ParsedPlotPayload | null, invalidContent: ReactNode = 'invalid') {
  act(() => root.render(
    <PlotResultView payload={payload} invalidContent={invalidContent} />,
  ));
}

beforeEach(() => {
  vi.clearAllMocks();
  host = document.createElement('div');
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe('PlotResultView', () => {
  it('renders the supplied localized invalid-format content for a null payload', () => {
    renderResult(null, <span data-invalid-content>localized invalid plot</span>);

    expect(host.querySelector('[data-invalid-content]')?.textContent)
      .toBe('localized invalid plot');
    expect(host.querySelector('[data-result-renderer]')).toBeNull();
  });

  it('sends normalized line models to the line toolbar controller', () => {
    const payload: ParsedPlotPayload = {
      kind: 'line',
      data: {
        data: [{ x: 1, y: 7 }],
        xLabel: 'Date',
        yLabel: 'Revenue',
        xFormat: 'date',
        yFormat: 'number',
      },
    };

    renderResult(payload);

    expect(host.querySelector('[data-result-renderer="line-controls"]')).not.toBeNull();
    expect(calls.lineControls).toHaveBeenCalledWith({
      model: {
        kind: 'line',
        points: payload.data.data,
        xAxis: { label: 'Date', valueType: 'date' },
        yAxis: { label: 'Revenue', valueType: 'number' },
        showPoints: true,
      },
    });
  });

  it('renders non-line models without line controls', () => {
    const payload: ParsedPlotPayload = {
      kind: 'scatter',
      data: {
        data: [{ x: 2, y: 8 }],
        xLabel: 'X',
        yLabel: 'Y',
      },
    };

    renderResult(payload);

    expect(host.querySelector('[data-result-renderer="scatter"]')).not.toBeNull();
    expect(calls.lineControls).not.toHaveBeenCalled();
  });

  it('adds correlogram labels and colors outside the source DTO', () => {
    const acf = [{ lag: 0, value: 1, qStat: 0, pValue: 1 }];
    const pacf = [{ lag: 1, value: 0.35, qStat: 1.2, pValue: 0.3 }];
    const payload: ParsedPlotPayload = {
      kind: 'correlogram',
      data: { acf, pacf, ciHalfWidth: 0.2, n: 40 },
    };

    renderResult(payload);

    expect(calls.correlogram).toHaveBeenCalledTimes(2);
    expect(calls.correlogram.mock.calls.map(([props]) => props)).toEqual([
      {
        data: acf,
        ciHalfWidth: 0.2,
        title: 'ACF',
        valueLabel: 'ACF',
      },
      {
        data: pacf,
        ciHalfWidth: 0.2,
        title: 'PACF',
        color: 'result-secondary',
        valueLabel: 'PACF',
      },
    ]);
  });
});
