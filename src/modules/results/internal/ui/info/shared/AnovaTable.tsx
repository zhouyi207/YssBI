import { DataTable } from "@/components/ui-presentation/DataTable";
import type { UiTableColumn } from "@/shared/types/domain/uiData";
import type { LinearModelInfo } from "@/shared/types/report";

const columns: readonly UiTableColumn[] = [
  { id: "source", label: "Source", format: "text" },
  { id: "ss", label: "SS", format: "number" },
  { id: "df", label: "df", format: "integer" },
  { id: "ms", label: "MS", format: "number" },
];

export function AnovaTable({ info }: { info: LinearModelInfo }) {
  return (
    <DataTable
      columns={columns}
      rows={[
        ["Model", info.ss_model, info.df_model, info.ms_model],
        ["Residual", info.ss_residual, info.df_residual, info.ms_residual],
        ["Total", info.ss_total, info.df_total, info.ms_total],
      ]}
    />
  );
}
