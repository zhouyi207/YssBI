import { useContext, useEffect } from "react";
import {
  GroupContext,
  useEditorGroupWorkspace,
} from "@/features/application/editor/editorGroupContext";
import { useWorksheetRead } from "@/features/core/worksheet/read";
import { loadWorksheetDocumentForView } from "@/features/application/worksheet/worksheetViewActions";
import { WorksheetChartPreview } from "./WorksheetChartPreview";
import { WorksheetEmptyState } from "./WorksheetEmptyState";

export function WorksheetEditor() {
  const groupId = useContext(GroupContext);
  const { activeTabId } = useEditorGroupWorkspace(groupId);
  const document = useWorksheetRead((snapshot) =>
    activeTabId ? (snapshot.documents[activeTabId] ?? null) : null,
  );
  const hasActiveDocument = useWorksheetRead((snapshot) =>
    activeTabId ? Boolean(snapshot.documents[activeTabId]) : false,
  );

  useEffect(() => {
    if (!activeTabId) return;
    if (hasActiveDocument) return;
    void loadWorksheetDocumentForView(activeTabId);
  }, [activeTabId, hasActiveDocument]);

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
