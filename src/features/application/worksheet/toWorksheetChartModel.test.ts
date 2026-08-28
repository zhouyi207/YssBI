import { describe, expect, it } from 'vitest';
import type { PlotColumnPairPayload, WorksheetPreviewPayload } from '@/shared/types/domain';
import { toWorksheetChartModel } from './toWorksheetChartModel';

describe('toWorksheetChartModel', () => {
  it('maps a Worksheet line preview to line data-space semantics', () => {
    const pair: PlotColumnPairPayload = {
      data: [{ x: 2, y: 8 }],
      xLabel: 'Date',
      yLabel: 'Revenue',
      xFormat: 'date',
      yFormat: 'number',
    };

    expect(toWorksheetChartModel({ kind: 'line', pair })).toMatchObject({
      kind: 'line',
      points: pair.data,
      xAxis: { label: 'Date', valueType: 'date' },
      yAxis: { label: 'Revenue', valueType: 'number' },
      showPoints: true,
    });
  });

  it.each<WorksheetPreviewPayload>([
    { kind: 'empty' },
    { kind: 'error', code: 'worksheet_preview_failed', incidentId: 'incident-1' },
  ])('leaves the $kind preview state to the Worksheet view', (payload) => {
    expect(toWorksheetChartModel(payload)).toBeNull();
  });
});
