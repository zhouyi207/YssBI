import { describe, expect, it } from 'vitest';
import { createDefaultBayesDraft, createEmptyBayesDraft } from '@/features/domain/bayes';
import { composeLikelihoodLatex, currentResponseExpression, latexSymbol } from './BayesPanels';
import { essRating, filterDensitySeries, filterTraceSeries, posteriorPredictiveChartData, rhatRating, traceChains } from './BayesResultPanels';

describe('parameter diagnostic ratings', () => {
  it('rates R-hat and ESS at user-facing severity boundaries', () => {
    expect(rhatRating(1.001).code).toBe('recommended');
        expect(rhatRating(1.06).code).toBe('untrustworthy');
        expect(essRating(80).code).toBe('unreliable');
        expect(essRating(399).code).toBe('low');
        expect(essRating(400).code).toBe('acceptable');
        expect(essRating(undefined).code).toBe('unavailable');
  });
});

describe('posterior predictive chart projection', () => {
  it('maps predictive quantiles to interval bounds without changing observations', () => {
    const rows = [{
      observation: 7,
      model: { observed: 1.6, mean: 1.7, q025: 1.4, q975: 1.9 },
      original: { observed: 5.1, mean: 5.3, q025: 4.4, q975: 6.2 },
    }];

    expect(posteriorPredictiveChartData(rows, 'original')).toEqual([{
      observation: 7,
      observed: 5.1,
      mean: 5.3,
      lower: 4.4,
      upper: 6.2,
    }]);
    expect(posteriorPredictiveChartData(rows, 'model')[0]?.mean).toBe(1.7);
  });
});

describe('posterior trace chain selection', () => {
  const series = [
    { parameter: 'a', chain: 2, points: [{ draw: 1, value: 2 }] },
    { parameter: 'a', chain: 1, points: [{ draw: 1, value: 1 }] },
    { parameter: 'a', chain: 2, points: [{ draw: 2, value: 3 }] },
  ];

  it('derives unique sorted chain options from trace data', () => {
    expect(traceChains(series)).toEqual([1, 2]);
  });

  it('shows all series by default and only the selected chain on demand', () => {
    expect(filterTraceSeries(series, '__all__')).toEqual(series);
    expect(filterTraceSeries(series, '2')).toEqual([series[0], series[2]]);
  });
});

describe('posterior density chain selection', () => {
  const density = [
    { parameter: 'a', chain: null, points: [{ x: 0, density: 0.5 }] },
    { parameter: 'a', chain: 1, points: [{ x: 0, density: 0.4 }] },
    { parameter: 'a', chain: 2, points: [{ x: 0, density: 0.6 }] },
  ];

  it('keeps pooled density separate from all-chain overlays', () => {
    expect(filterDensitySeries(density, '__pooled__')).toEqual([density[0]]);
    expect(filterDensitySeries(density, '__all__')).toEqual([density[1], density[2]]);
    expect(filterDensitySeries(density, '2')).toEqual([density[2]]);
  });
});

describe('Bayesian symbol LaTeX mapping', () => {
  it('maps known Greek parameter names without changing ordinary symbols', () => {
    expect(latexSymbol('sigma')).toBe('\\sigma');
    expect(latexSymbol('beta_0')).toBe('\\beta_{0}');
    expect(latexSymbol('beta_{12}')).toBe('\\beta_{12}');
    expect(latexSymbol('x_1')).toBe('x_{1}');
    expect(latexSymbol('a')).toBe('a');
  });
});

describe('FormulaStep response expression', () => {
  it('formats the stored raw response when editing', () => {
    const draft = createEmptyBayesDraft();
    draft.rawResponse = {
      type: 'call',
      function: 'ln',
      args: [{ type: 'symbol', name: 'y' }],
    };

    expect(currentResponseExpression(draft)).toBe('\\ln\\left(y\\right)');
  });

  it('provides one canonical LaTeX default formula', () => {
    const draft = createDefaultBayesDraft();

    expect(draft.symbols.map(symbol => symbol.name)).toEqual(['a', 'b', 'sigma', 'x', 'y']);
    expect(currentResponseExpression(draft)).toBe('y');
  });

  it('uses the complete response expression as the formula left side', () => {
    expect(composeLikelihoodLatex('\\ln y', 'normal', ['a \\cdot x + b', '\\sigma']))
      .toBe('\\ln y \\sim \\operatorname{Normal}\\left(a \\cdot x + b, \\sigma\\right)');
  });
});
