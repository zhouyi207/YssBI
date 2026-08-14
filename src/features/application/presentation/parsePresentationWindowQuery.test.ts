import { describe, expect, it } from 'vitest';
import { parsePresentationWindowQueryFromParts } from './parsePresentationWindowQuery';

describe('parsePresentationWindowQueryFromParts', () => {
  it('parses resultId and plotType from hash query', () => {
    const resultId = '9007199254740993';
    const params = new URLSearchParams({ resultId, plotType: 'scatter' });

    expect(parsePresentationWindowQueryFromParts(`#/inspect?${params.toString()}`)).toEqual({
      resultId,
      plotType: 'scatter',
    });
  });

});
