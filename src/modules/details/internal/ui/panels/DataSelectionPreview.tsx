import { useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { useEditorPaneStateStore, workbenchLayoutRead } from "@/modules/workbench/public";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";
import { DetailForm, DetailTextarea } from "../shared/DetailForm";

export function DataSelectionPreview({ databaseId }: { databaseId: string }) {
  const { t } = useTranslation();
  useSyncExternalStore(
    workbenchLayoutRead.subscribeActivePanel,
    workbenchLayoutRead.getActiveSnapshot,
    workbenchLayoutRead.getActiveSnapshot,
  );
  const panel = workbenchLayoutRead.getActiveEditorPanel();
  const panelInstanceId =
    panel?.metadata.resourceKind === "database" && panel.metadata.resourceRef === databaseId
      ? panel.panelInstanceId
      : undefined;
  const selectedCellText = useEditorPaneStateStore((state) => {
    const view = panelInstanceId ? state.databaseViews[panelInstanceId] : undefined;
    return view?.databaseId === databaseId ? view.selectedCellText : "";
  });

  return (
    <DetailCollapsibleSection title={t("databaseEditor.selectedContent")}>
      <DetailForm>
        <DetailTextarea
          readOnly
          value={selectedCellText}
          rows={5}
          aria-label={t("databaseEditor.selectedContent")}
          placeholder={t("databaseEditor.cellPreviewPlaceholder")}
          className="font-mono text-xs"
        />
      </DetailForm>
    </DetailCollapsibleSection>
  );
}
