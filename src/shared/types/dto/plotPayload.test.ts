import { describe, expect, it } from 'vitest';
import {
  parseCorrelationPlot,
  parseCorrelogramPlot,
  parseHistogramPlot,
  parsePlotPayload,
  parseXySeriesPlot,
} from './plotPayload';

describe('parseXySeriesPlot', () => {
  it('accepts camelCase scatter payload from Rust', () => {
    const result = parseXySeriesPlot({
      data: [{ x: 1, y: 2 }],
      xLabel: 'X',
      yLabel: 'Y',
      xFormat: 'date',
      yFormat: 'number',
    });
    expect(result).toEqual({
      data: [{ x: 1, y: 2 }],
      xLabel: 'X',
      yLabel: 'Y',
      xFormat: 'date',
      yFormat: 'number',
    });
  });

  it('accepts snake_case ecdf payload from Rust', () => {
    const result = parseXySeriesPlot({
      data: [{ x: 0.5, y: 0.25 }],
      x_label: 'Value',
      y_label: 'ECDF',
    });
    expect(result?.xLabel).toBe('Value');
    expect(result?.yLabel).toBe('ECDF');
  });

  it('rejects empty or invalid points', () => {
    expect(parseXySeriesPlot({ data: [] })).toBeNull();
    expect(parseXySeriesPlot({ data: [{ x: 'a', y: 1 }] })).toBeNull();
    expect(parseXySeriesPlot(null)).toBeNull();
  });
});

describe('parseHistogramPlot', () => {
  it('parses histogram bins', () => {
    const result = parseHistogramPlot({
      data: [{ label: '[0, 1)', count: 3 }],
      x_label: 'x',
      y_label: 'Frequency',
    });
    expect(result).toEqual({
      data: [{ label: '[0, 1)', count: 3 }],
      xLabel: 'x',
      yLabel: 'Frequency',
    });
  });

  it('rejects non-integer counts', () => {
    expect(
      parseHistogramPlot({
        data: [{ label: 'a', count: 1.5 }],
      }),
    ).toBeNull();
  });
});

describe('parseCorrelogramPlot', () => {
  it('parses acf/pacf with ci and n', () => {
    const result = parseCorrelogramPlot({
      acf: [{ lag: 1, value: 0.5, q_stat: 1.2, p_value: 0.3 }],
      pacf: [{ lag: 1, value: 0.4, q_stat: 1.1, p_value: 0.25 }],
      ci_half_width: 0.2,
      n: 100,
    });
    expect(result).toEqual({
      acf: [{ lag: 1, value: 0.5, q_stat: 1.2, p_value: 0.3 }],
      pacf: [{ lag: 1, value: 0.4, q_stat: 1.1, p_value: 0.25 }],
      ciHalfWidth: 0.2,
      n: 100,
    });
  });

  it('rejects plot bar missing ljung-box stats', () => {
    expect(
      parseCorrelogramPlot({
        acf: [{ lag: 1, value: 0.5 }],
        pacf: [{ lag: 1, value: 0.4, q_stat: 1, p_value: 0.1 }],
        ci_half_width: 0.2,
        n: 100,
      }),
    ).toBeNull();
  });
});

describe('parseCorrelationPlot', () => {
  it('parses square matrix with optional p_matrix', () => {
    const result = parseCorrelationPlot({
      labels: ['A', 'B'],
      matrix: [
        [1, 0.5],
        [0.5, 1],
      ],
      p_matrix: [
        [0, 0.1],
        [0.1, 0],
      ],
    });
    expect(result?.labels).toEqual(['A', 'B']);
    expect(result?.pMatrix?.[0][1]).toBe(0.1);
  });

  it('rejects non-square matrix', () => {
    expect(
      parseCorrelationPlot({
        labels: ['A', 'B'],
        matrix: [[1]],
      }),
    ).toBeNull();
  });
});

describe('parsePlotPayload', () => {
  it('dispatches by chart kind', () => {
    expect(
      parsePlotPayload('scatter', {
        data: [{ x: 1, y: 2 }],
      })?.kind,
    ).toBe('scatter');
    expect(
      parsePlotPayload('histogram', {
        data: [{ label: 'a', count: 1 }],
      })?.kind,
    ).toBe('histogram');
    expect(parsePlotPayload('scatter', { data: [] })).toBeNull();
  });
});
