import { describe, expect, it } from 'vitest';
import { parsePresentationWindowQueryFromParts } from './parsePresentationWindowQuery';

describe('parsePresentationWindowQueryFromParts', () => {
  it('parses sourceId and plotType from hash query', () => {
    const sourceId = 'runtime_abc_events/Main.yssbi-event_pin-a';
    const params = new URLSearchParams({ sourceId, plotType: 'scatter' });

    expect(parsePresentationWindowQueryFromParts(`#/inspect?${params.toString()}`)).toEqual({
      sourceId,
      plotType: 'scatter',
    });
  });
});
