import { describe, expect, it } from 'vitest';
import type { BayesModelDraftDTO, LikelihoodSpecDTO } from '@/shared/types/bayes';
import { createEmptyBayesDraft } from '@/features/domain/bayes';
import { normalizeIpcError } from '@/services/ipc';
import { buildFormulaParseRequest, formatFormulaParseError } from './formulaParsing';

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

  it('keeps only normalized code, safe details, and incident ID', () => {
    const error = normalizeIpcError('parse_bayes_expression', {
      code: 'bayes_expression_parse_failed',
      details: { column: 'formula', row: 2, path: 'formulaText' },
      incidentId: 'incident-formula-42',
    });

    expect(formatFormulaParseError(error)).toEqual({
      code: 'bayes_expression_parse_failed',
      details: { column: 'formula', row: 2, path: 'formulaText' },
      incidentId: 'incident-formula-42',
    });
  });

  it('drops legacy backend prose and never copies a raw Error message', () => {
    const legacy = normalizeIpcError('parse_bayes_expression', {
      code: 'bayes_expression_parse_failed',
      details: { detail: 'private parser prose' },
      incidentId: null,
    });

    expect(formatFormulaParseError(legacy)).toEqual({
      code: 'bayes_expression_parse_failed',
      details: null,
      incidentId: null,
    });
    expect(formatFormulaParseError(new Error('private transport failure'))).toEqual({
      code: 'bayes_expression_parse_failed',
      details: null,
      incidentId: null,
    });
  });

});
