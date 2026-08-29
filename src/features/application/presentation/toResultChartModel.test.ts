import { describe, expect, it } from 'vitest';
import { isResultPlotKind } from '@/shared/types/dto/result';
import type { ParsedPlotPayload, XySeriesPlotDTO } from '@/shared/types/dto/plotPayload';
import { toResultChartModel } from './toResultChartModel';

describe('isResultPlotKind', () => {
  it('recognizes canonical Result plot kinds only', () => {
    expect(isResultPlotKind('correlation')).toBe(true);
    expect(isResultPlotKind('unknown')).toBe(false);
  });
});

describe('toResultChartModel', () => {
  it('normalizes a generic plot payload to scatter data-space semantics', () => {
    const xyPayload: XySeriesPlotDTO = {
      data: [{ x: 1, y: 4 }],
      xLabel: 'Time',
      yLabel: 'Value',
      yFormat: 'date',
    };
    const payload: ParsedPlotPayload = { kind: 'plot', data: xyPayload };

    expect(toResultChartModel(payload)).toEqual({
      kind: 'scatter',
      points: xyPayload.data,
      xAxis: { label: xyPayload.xLabel, valueType: 'number' },
      yAxis: { label: xyPayload.yLabel, valueType: 'date' },
    });
  });
});
