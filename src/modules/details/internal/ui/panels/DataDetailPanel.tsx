import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/shared/ui";
import { useDatabaseMetadata } from "@/features/application/dataManagement/databaseRead";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { DetailPanelShell } from "../shared/DetailPanelShell";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";
import { DetailFieldRow } from "../shared/DetailFieldRow";
import { DataColumnSettings } from "./DataColumnSettings";
import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";

interface DataDetailPanelProps {
  dataframe: DeepReadonly<DatabaseRecord>;
}

export function DataDetailPanel({ dataframe }: DataDetailPanelProps) {
  const { t } = useTranslation();
  const columnCount = dataframe.columnCount ?? dataframe.columns?.length;
  const rowCount = dataframe.rowCount;
  const [selected, setSelected] = useState("");
  const column =
    dataframe.columns?.find((column) => column.name === selected) ?? dataframe.columns?.[0];
  const revision = useDatabaseMetadata(
    dataframe.id,
    dataframe.columns !== undefined && dataframe.rowCount !== undefined,
  );

  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField label={t("detail.fields.name")} tone="body">
          {dataframe.name}
        </DetailReadonlyField>
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
              key={`${dataframe.id}:${column.name}:${revision}`}
              databaseId={dataframe.id}
              column={column}
            />
          )}
        </DetailCollapsibleSection>
      )}
    </DetailPanelShell>
  );
}
