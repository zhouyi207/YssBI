import { describe, expect, it } from 'vitest';
import type { BayesModelDraftDTO, LikelihoodSpecDTO } from '@/shared/types/bayes';
import { createEmptyBayesDraft } from '@/features/domain/bayes';
import { buildFormulaParseRequest } from './formulaParsing';

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


describe('Bayes formula parsing state', () => {
  it('includes dataset and symbol context in the request', () => {
    expect(buildFormulaParseRequest(draftWithContext(), '\\ln target = x', normalLikelihood)).toEqual({
          formula: '\\ln target = x',
          columns: [{ name: 'observed_x', dtype: 'number', nullable: false }],
          symbols: ['x', 'observed_x', 'sigma'],
    });
  });


});
