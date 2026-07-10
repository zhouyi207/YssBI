import { useMemo } from 'react';
import { useEditorSessionResources } from '@/features/application/editor';
import { useDetailTarget } from '@/features/core/editor';
import { useLogStore } from '@/features/core/log/logStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { resolveDetailPanelModel } from './resolveDetailPanelModel';
import type { DetailPanelModel } from './resolveDetailPanelModel';

export function useDetailPanelModel(): {
  model: DetailPanelModel;
  worksheetTargetId: string | null;
  worksheetDocument: ReturnType<typeof useWorksheetStore.getState>['documents'][string] | null;
} {
  const { variables, events, functions, dataframes } = useEditorSessionResources();
  const target = useDetailTarget();
  const selectedLog = useLogStore((s) => s.selectedLog);

  const worksheetTargetId =
    target?.kind === 'worksheet' && 'id' in target ? target.id : null;

  const worksheetDocument = useWorksheetStore((s) =>
    worksheetTargetId ? s.documents[worksheetTargetId] ?? null : null,
  );

  const model = useMemo(
    () =>
      resolveDetailPanelModel({
        target,
        selectedLog,
        variables,
        events,
        functions,
        dataframes,
        worksheetDocument,
      }),
    [target, selectedLog, variables, events, functions, dataframes, worksheetDocument],
  );

  return { model, worksheetTargetId, worksheetDocument };
}
