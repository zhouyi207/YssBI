import { useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { selectDatabaseNames, useDatabaseRead } from "@/features/core/database/read";
import { useDatabaseMetadata } from "@/features/application/dataManagement/databaseRead";
import { chartUi } from "@/features/core/chart/ui";
import { useChartDocumentLoad } from "@/features/application/chart/useChartDocumentLoad";
import { Select } from "@/shared/ui";
import type { ChartType, ChartDocument } from "@/shared/types/domain/chart";
import type { ColumnInfo } from "@/shared/types/domain/database";
import { DetailPanelShell } from "../shared/DetailPanelShell";
import { DetailFieldRow } from "../shared/DetailFieldRow";
import { DetailColumnList } from "../shared/DetailColumnList";
import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";
import { DetailSectionHeader } from "../shared/DetailText";
import { FileDetailPanel } from "./FileDetailPanel";

const CHART_TYPES: ChartType[] = ["histogram", "scatter", "line"];
const EMPTY_COLUMNS: readonly ColumnInfo[] = [];

function isNumericType(type: string): boolean {
  const t = type.toLowerCase();
  return (
    t.includes("int") ||
    t.includes("float") ||
    t.includes("double") ||
    t.includes("decimal") ||
    t.includes("number") ||
    t.includes("date")
  );
}

interface ChartDetailPanelProps {
  chartPath: string;
  name: string;
  document: ChartDocument;
}

export function ChartDetailPanel(
  props: Omit<ChartDetailPanelProps, "document"> & { document: ChartDocument | null },
) {
  const { chartPath, document } = props;
  const { failed, retry } = useChartDocumentLoad(chartPath, Boolean(document));
  if (!document)
    return (
      <FileDetailPanel
        resourceKind="chart"
        resourceRef={chartPath}
        status={failed ? "error" : "loading"}
        onRetry={retry}
      />
    );
  return <ChartDetailForm {...props} document={document} />;
}

function ChartDetailForm({ chartPath, name, document }: ChartDetailPanelProps) {
  const { t } = useTranslation();
  const databaseNames = useDatabaseRead(useShallow(selectDatabaseNames));
  const databaseOptions = useMemo(
    () => Object.entries(databaseNames).map(([value, label]) => ({ value, label })),
    [databaseNames],
  );
  const columns = useDatabaseRead((snapshot) => snapshot.databases[document.databaseId]?.columns);
  useDatabaseMetadata(document.databaseId || undefined, columns !== undefined);

  const { allColumnOptions, numericColumnOptions } = useMemo(() => {
    const allColumnOptions: { label: string; value: string }[] = [];
    const numericColumnOptions: { label: string; value: string }[] = [];
    for (const column of columns ?? EMPTY_COLUMNS) {
      const option = { label: column.name, value: column.name };
      allColumnOptions.push(option);
      if (
        column.semantic
          ? column.semantic.kind === "Numeric" || column.semantic.kind === "Datetime"
          : isNumericType(column.type)
      )
        numericColumnOptions.push(option);
    }
    return { allColumnOptions, numericColumnOptions };
  }, [columns]);

  const patch = (changes: Parameters<typeof chartUi.updateDraft>[1]) => {
    chartUi.updateDraft(chartPath, changes);
  };

  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField label={t("detail.fields.name")} tone="body">
          {name}
        </DetailReadonlyField>
        <DetailFieldRow label={t("chartsSidebar.dataset")}>
          <Select
            value={document.databaseId}
            options={databaseOptions}
            onChange={(val) => patch({ databaseId: val, encodings: {} })}
          />
        </DetailFieldRow>
        <DetailFieldRow label={t("chartsSidebar.chartType")}>
          <Select
            value={document.chartType}
            options={CHART_TYPES.map((type) => ({
              value: type,
              label: t(`chartsSidebar.chartTypes.${type}`),
            }))}
            onChange={(val) => patch({ chartType: val as ChartType, encodings: {} })}
          />
        </DetailFieldRow>
        {document.chartType === "histogram" ? (
          <DetailFieldRow label={t("chartsSidebar.encodingY")}>
            <Select
              value={document.encodings.y ?? ""}
              options={allColumnOptions}
              onChange={(val) => patch({ encodings: { ...document.encodings, y: val } })}
            />
          </DetailFieldRow>
        ) : (
          <>
            <DetailFieldRow label={t("chartsSidebar.encodingX")}>
              <Select
                value={document.encodings.x ?? ""}
                options={numericColumnOptions}
                onChange={(val) => patch({ encodings: { ...document.encodings, x: val } })}
              />
            </DetailFieldRow>
            <DetailFieldRow label={t("chartsSidebar.encodingY")}>
              <Select
                value={document.encodings.y ?? ""}
                options={numericColumnOptions}
                onChange={(val) => patch({ encodings: { ...document.encodings, y: val } })}
              />
            </DetailFieldRow>
          </>
        )}
      </DetailForm>

      <Card className="rounded-none border-0 bg-transparent py-0 shadow-none">
        <CardHeader className="h-7 border-0 px-3 py-0">
          <DetailSectionHeader level="subsection">{t("chartsSidebar.columns")}</DetailSectionHeader>
        </CardHeader>
        <CardContent className="px-3 pb-2 pt-1">
          <DetailColumnList
            columns={columns ?? EMPTY_COLUMNS}
            emptyMessage={t("chartsSidebar.noColumns")}
          />
        </CardContent>
      </Card>
    </DetailPanelShell>
  );
}
