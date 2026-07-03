import { useContext, useEffect } from 'react';
import { GroupContext } from '@/features/core/editor';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { WorksheetChartPreview } from './WorksheetChartPreview';
import { WorksheetEmptyState } from './WorksheetEmptyState';

export function WorksheetEditor() {
  const groupId = useContext(GroupContext);
  const activeTabId = useLayoutStore(
    (s) => (groupId ? s.nodes[groupId]?.data?.activeTabId : undefined),
  );
  const document = useWorksheetStore((s) =>
    activeTabId ? s.documents[activeTabId] ?? null : null,
  );

  useEffect(() => {
    if (!activeTabId) return;
    if (useWorksheetStore.getState().documents[activeTabId]) return;
    void WorksheetService.loadWorksheet(activeTabId)
      .then((loaded) => useWorksheetStore.getState().upsertDocument(loaded))
      .catch(() => undefined);
  }, [activeTabId]);

  if (!activeTabId) {
    return (
      <div className="flex h-full w-full min-h-0">
        <WorksheetEmptyState messageKey="worksheet.noActiveWorksheet" />
      </div>
    );
  }

  return (
    <div className="flex h-full w-full min-h-0 flex-col">
      <WorksheetChartPreview document={document} />
    </div>
  );
}
