import { DatabaseService } from '@/services/database/databaseService';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import type {
  ColumnDistribution,
  WorksheetDocument,
  WorksheetPreviewPayload,
} from '@/shared/types/domain';

export interface WorksheetPreviewProjectIdentity {
  readonly projectInstanceId: string;
  isCurrent(): boolean;
  assertCurrent(): void;
}

export async function fetchWorksheetPreview(
  document: WorksheetDocument,
  identity: WorksheetPreviewProjectIdentity,
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
        identity.projectInstanceId,
        databaseId,
      )) as ColumnDistribution[];
      identity.assertCurrent();
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

      const pair = await WorksheetService.getPlotColumnPair(
        identity.projectInstanceId,
        databaseId,
        xCol,
        yCol,
      );
      identity.assertCurrent();
      return { kind: chartType, pair };
    }

    return { kind: 'empty' };
  } catch (error) {
    if (!identity.isCurrent()) {
      identity.assertCurrent();
    }
    return {
      kind: 'error',
      message: error instanceof Error ? error.message : String(error),
    };
  }
}
