import { DatabaseService } from '@/services/database/databaseService';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import type {
  ColumnDistribution,
  WorksheetDocument,
  WorksheetPreviewPayload,
} from '@/shared/types/domain';

export async function fetchWorksheetPreview(
  document: WorksheetDocument,
): Promise<WorksheetPreviewPayload> {
  const { databaseId, chartType, encodings } = document;

  if (!databaseId) {
    return { kind: 'empty' };
  }

  try {
    if (chartType === 'histogram') {
      const column = encodings.y ?? encodings.x;
      if (!column) return { kind: 'empty' };

      const distributions = (await DatabaseService.getColumnDistribution(
        databaseId,
      )) as ColumnDistribution[];
      const match = distributions.find((d) => d.columnName === column);
      if (!match) {
        return { kind: 'error', message: `Column "${column}" not found` };
      }
      if (match.kind === 'numeric') {
        return {
          kind: 'histogram',
          bins: match.bins,
          xLabel: column,
          yLabel: 'Count',
        };
      }
      return {
        kind: 'histogram',
        bins: match.categories.map((c) => ({ label: c.label, count: c.value })),
        xLabel: column,
        yLabel: 'Count',
      };
    }

    if (chartType === 'scatter' || chartType === 'line') {
      const xCol = encodings.x;
      const yCol = encodings.y;
      if (!xCol || !yCol) return { kind: 'empty' };

      const pair = await WorksheetService.getPlotColumnPair(databaseId, xCol, yCol);
      return { kind: chartType, pair };
    }

    return { kind: 'empty' };
  } catch (error) {
    return {
      kind: 'error',
      message: error instanceof Error ? error.message : String(error),
    };
  }
}
