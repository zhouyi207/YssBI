import { useTranslation } from "react-i18next";
import { useDatabaseMetadata } from "@/features/application/dataManagement/databaseRead";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { DetailPanelShell } from "../shared/DetailPanelShell";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";
import { DataColumnSettings } from "./DataColumnSettings";
import { DataSelectionPreview } from "./DataSelectionPreview";
import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";

interface DataDetailPanelProps {
  dataframe: DeepReadonly<DatabaseRecord>;
}

export function DataDetailPanel({ dataframe }: DataDetailPanelProps) {
  const { t } = useTranslation();
  const columnCount = dataframe.columnCount ?? dataframe.columns?.length;
  const rowCount = dataframe.rowCount;
  const revision = useDatabaseMetadata(
    dataframe.id,
    dataframe.columns !== undefined && dataframe.rowCount !== undefined,
  );

  return (
    <DetailPanelShell>
      <DetailCollapsibleSection title={dataframe.name} defaultOpen>
        <DetailForm>
          {dataframe.loadFailed && (
            <p role="alert" className="text-xs text-destructive">
              {t("detail.loadFailed")}
            </p>
          )}
          <DetailReadonlyField label={t("detail.fields.columns")}>
            {columnCount === undefined ? "—" : t("detail.counts.columns", { count: columnCount })}
          </DetailReadonlyField>
          <DetailReadonlyField label={t("detail.fields.rows")}>
            {rowCount === undefined ? "—" : t("detail.counts.rows", { count: rowCount })}
          </DetailReadonlyField>
        </DetailForm>
      </DetailCollapsibleSection>
      {dataframe.columns && dataframe.columns.length > 0 && (
        <DetailCollapsibleSection title={t("detail.fields.columns")} defaultOpen>
          {dataframe.columns.map((column) => (
            <DetailCollapsibleSection key={column.name} title={column.name}>
              <DataColumnSettings
                key={`${dataframe.id}:${column.name}:${revision}`}
                databaseId={dataframe.id}
                column={column}
              />
            </DetailCollapsibleSection>
          ))}
        </DetailCollapsibleSection>
      )}
      <DataSelectionPreview databaseId={dataframe.id} />
    </DetailPanelShell>
  );
}
