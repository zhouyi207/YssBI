import { describe, expect, it } from 'vitest';
import { createDefaultBayesDraft, createEmptyBayesDraft, DEFAULT_BAYES_FORMULA } from '@/features/domain/bayes';
import { composeLikelihoodLatex, currentResponseExpression, latexSymbol } from './BayesPanels';
import { essRating, filterTraceSeries, posteriorPredictiveChartData, rhatRating, traceChains } from './BayesResultPanels';

describe('parameter diagnostic ratings', () => {
  it('rates R-hat and ESS at user-facing severity boundaries', () => {
    expect(rhatRating(1.001).label).toBe('推荐标准');
    expect(rhatRating(1.06).label).toBe('不建议相信');
    expect(essRating(80).label).toBe('不可靠');
    expect(essRating(399).label).toBe('偏低');
    expect(essRating(400).label).toBe('可接受');
    expect(essRating(undefined).label).toBe('不可用');
  });
});

describe('posterior predictive chart projection', () => {
  it('maps predictive quantiles to interval bounds without changing observations', () => {
    expect(posteriorPredictiveChartData([{
      observation: 7,
      observed: 5.1,
      mean: 5.3,
      q025: 4.4,
      q975: 6.2,
    }])).toEqual([{
      observation: 7,
      observed: 5.1,
      mean: 5.3,
      lower: 4.4,
      upper: 6.2,
    }]);
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

describe('Bayesian symbol LaTeX mapping', () => {
  it('maps known Greek parameter names without changing ordinary symbols', () => {
    expect(latexSymbol('sigma')).toBe('\\sigma');
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

    expect(draft.formulaText).toBe(DEFAULT_BAYES_FORMULA);
    expect(draft.symbols.map(symbol => symbol.name)).toEqual(['a', 'b', 'sigma', 'x', 'y']);
    expect(currentResponseExpression(draft)).toBe('y');
  });

  it('uses the complete response expression as the formula left side', () => {
    expect(composeLikelihoodLatex('\\ln y', 'normal', ['a \\cdot x + b', '\\sigma']))
      .toBe('\\ln y \\sim \\operatorname{Normal}\\left(a \\cdot x + b, \\sigma\\right)');
  });
});
