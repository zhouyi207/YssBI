import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/shared/ui";
import { hydrateDatabaseEditorMetadata } from "@/features/application/dataManagement/databaseRecords";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { DetailPanelShell } from "../shared/DetailPanelShell";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";
import { DetailFieldRow } from "../shared/DetailFieldRow";
import { DataColumnSettings } from "./DataColumnSettings";
import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";

interface DataDetailPanelProps {
  dataframe: DatabaseRecord;
}

export function DataDetailPanel({ dataframe }: DataDetailPanelProps) {
  const { t } = useTranslation();
  const columnCount = dataframe.columnCount ?? dataframe.columns?.length ?? 0;
  const rowCount = dataframe.rowCount ?? 0;
  const [selected, setSelected] = useState("");
  const column =
    dataframe.columns?.find((column) => column.name === selected) ?? dataframe.columns?.[0];
  const hasSemantics = Boolean(dataframe.columns?.every((column) => column.semantic));
  useEffect(() => {
    if (hasSemantics) return;
    let cancelled = false;
    void hydrateDatabaseEditorMetadata(dataframe.id, () => cancelled);
    return () => {
      cancelled = true;
    };
  }, [dataframe.id, hasSemantics]);

  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField label={t("detail.fields.name")} tone="body">
          {dataframe.name}
        </DetailReadonlyField>
        <DetailReadonlyField label={t("detail.fields.columns")}>
          {t("detail.counts.columns", { count: columnCount })}
        </DetailReadonlyField>
        <DetailReadonlyField label={t("detail.fields.rows")}>
          {t("detail.counts.rows", { count: rowCount })}
        </DetailReadonlyField>
      </DetailForm>
      {dataframe.columns && dataframe.columns.length > 0 && (
        <DetailCollapsibleSection title={t("detail.fields.columns")}>
          <DetailForm>
            <DetailFieldRow label={t("detail.fields.column")}>
              <Select
                value={column?.name ?? ""}
                options={dataframe.columns.map((column) => ({
                  value: column.name,
                  label: column.name,
                }))}
                onChange={setSelected}
              />
            </DetailFieldRow>
          </DetailForm>
          {column && (
            <DataColumnSettings
              key={`${dataframe.id}:${column.name}:${JSON.stringify(column)}`}
              databaseId={dataframe.id}
              column={column}
            />
          )}
        </DetailCollapsibleSection>
      )}
    </DetailPanelShell>
  );
}
