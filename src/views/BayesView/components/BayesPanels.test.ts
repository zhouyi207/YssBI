import { describe, expect, it } from 'vitest';
import { createDefaultBayesDraft, createEmptyBayesDraft, DEFAULT_BAYES_FORMULA } from '@/features/domain/bayes';
import { composeLikelihoodLatex, currentResponseExpression } from './BayesPanels';

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
