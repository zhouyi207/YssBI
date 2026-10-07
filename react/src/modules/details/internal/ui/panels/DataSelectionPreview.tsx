import { useId, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { useEditorPaneStateStore, workbenchLayoutRead } from "@/modules/workbench/public";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";
import { DetailFieldRow } from "../shared/DetailFieldRow";
import { DetailForm, DetailReadonlyField, DetailTextarea } from "../shared/DetailForm";

export function DataSelectionPreview({ databaseId }: { databaseId: string }) {
  const { t } = useTranslation();
  const contentId = useId();
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
  const selected = useEditorPaneStateStore((state) => {
    const view = panelInstanceId ? state.databaseViews[panelInstanceId] : undefined;
    return view?.databaseId === databaseId ? view : undefined;
  });
  const contentPlaceholder =
    selected?.selectedRowNumber == null
      ? t("databaseEditor.cellPreviewPlaceholder")
      : selected.selectedCellText === null
        ? t("databaseEditor.nullCellPlaceholder")
        : t("databaseEditor.emptyStringPlaceholder");

  return (
    <DetailCollapsibleSection title={t("databaseEditor.selected")}>
      <DetailForm>
        <DetailReadonlyField label={t("databaseEditor.rowNumber")} tone="mono">
          {selected?.selectedRowNumber ?? "—"}
        </DetailReadonlyField>
        <DetailReadonlyField label={t("databaseEditor.columnName")} tone="body">
          <span className="min-w-0 truncate" title={selected?.selectedColumnName ?? undefined}>
            {selected?.selectedColumnName ?? "—"}
          </span>
        </DetailReadonlyField>
        <DetailFieldRow label={t("databaseEditor.content")} htmlFor={contentId} layout="stacked">
          <DetailTextarea
            id={contentId}
            readOnly
            value={selected?.selectedCellText ?? ""}
            rows={5}
            placeholder={contentPlaceholder}
            className="font-mono text-xs"
          />
        </DetailFieldRow>
      </DetailForm>
    </DetailCollapsibleSection>
  );
}
