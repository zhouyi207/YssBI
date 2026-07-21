import { describe, expect, it } from 'vitest';
import type { BayesModelDraftDTO, LikelihoodSpecDTO } from '@/shared/types/bayes';
import { createEmptyBayesDraft } from '@/features/domain/bayes';
import {
  buildFormulaParseRequest,
  formulaParseReducer,
  restoreParsedSymbols,
  type FormulaParseState,
} from './formulaParsing';

const normalLikelihood: LikelihoodSpecDTO = {
  type: 'normal',
  mean: { source: 'predictor' },
  sigma: { parameter: 'sigma' },
};

function draftWithContext(): BayesModelDraftDTO {
  return {
    ...createEmptyBayesDraft(),
    symbols: [{ name: 'x', role: 'independent', inferredRole: 'independent', userEdited: false }],
    dataset: {
      sourceType: 'table',
      sourceId: 'dataset-1',
      columns: [{ name: 'observed_x', dtype: 'number', nullable: false }],
    },
  };
}

const initialState: FormulaParseState = {
  generation: 0,
  formula: {
    formulaText: 'y = old',
    rawResponse: { type: 'symbol', name: 'y' },
    rawPredictor: { type: 'symbol', name: 'old' },
  },
  error: null,
};

describe('Bayes formula parsing state', () => {
  it('includes dataset and symbol context in the request', () => {
    expect(buildFormulaParseRequest(draftWithContext(), '\\ln target = x', normalLikelihood)).toEqual({
          formula: '\\ln target = x',
          columns: [{ name: 'observed_x', dtype: 'number', nullable: false }],
          symbols: ['x', 'observed_x', 'sigma'],
    });
  });

  it('restores symbols that the user adds back through the formula', () => {
    expect(restoreParsedSymbols(new Set(['a', 'old']), ['y', 'a', 'x'])).toEqual(new Set(['old']));
  });


  it('ignores a response from an older generation', () => {
    const editing = formulaParseReducer(initialState, { type: 'started', generation: 2, formulaText: 'y = newest' });
    const stale = formulaParseReducer(editing, {
      type: 'succeeded',
      generation: 1,
      response: {
              formula: {
                formulaText: 'y = stale',
                rawResponse: { type: 'symbol', name: 'y' },
                rawPredictor: { type: 'symbol', name: 'stale' },
              },
              symbols: ['y', 'stale'],
            },
    });

    expect(stale).toBe(editing);
    expect(stale.formula.formulaText).toBe('y = newest');
  });

  it('keeps edited text and clears the old AST after failure', () => {
    const editing = formulaParseReducer(initialState, { type: 'started', generation: 1, formulaText: 'y = broken(' });
    const failed = formulaParseReducer(editing, {
      type: 'failed',
      generation: 1,
      error: { code: 'INVALID_EXPRESSION', message: 'Expected closing parenthesis' },
    });

    expect(failed.formula).toEqual({ formulaText: 'y = broken(', rawResponse: null, rawPredictor: null });
    expect(failed.error?.code).toBe('INVALID_EXPRESSION');
  });
});
