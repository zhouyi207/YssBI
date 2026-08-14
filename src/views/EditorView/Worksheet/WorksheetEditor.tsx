import { useContext, useEffect } from 'react';
import { GroupContext } from '@/features/core/editor';
import { useEditorGroupWorkspace } from '@/features/core/editor';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { WorksheetChartPreview } from './WorksheetChartPreview';
import { WorksheetEmptyState } from './WorksheetEmptyState';

export function WorksheetEditor() {
  const groupId = useContext(GroupContext);
  const { activeTabId } = useEditorGroupWorkspace(groupId);
  const document = useWorksheetStore((s) =>
    activeTabId ? s.documents[activeTabId] ?? null : null,
  );

  useEffect(() => {
    if (!activeTabId) return;
    if (useWorksheetStore.getState().documents[activeTabId]) return;
    const context = captureProjectCommandContext();
    void WorksheetService.loadWorksheet(context.projectInstanceId, activeTabId)
      .then((loaded) => {
        if (context.isCurrent()) {
          useWorksheetStore.getState().upsertDocument(activeTabId, loaded);
        }
      })
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
      <WorksheetChartPreview worksheetPath={activeTabId} document={document} />
    </div>
  );
}
