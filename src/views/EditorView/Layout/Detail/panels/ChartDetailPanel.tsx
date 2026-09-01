import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { useEditorSessionResources } from "@/features/application/editor";
import { hydrateDatabaseEditorMetadata } from "@/features/application/dataManagement/databaseRecords";
import { chartUi } from "@/features/core/chart/ui";
import { Select } from "@/shared/ui";
import type { ChartType, ChartDocument } from "@/shared/types/domain/chart";
import { DetailPanelShell } from "../shared/DetailPanelShell";
import { DetailFieldRow } from "../shared/DetailFieldRow";
import { DetailColumnList } from "../shared/DetailColumnList";
import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";
import { DetailSectionHeader } from "../shared/DetailText";

const CHART_TYPES: ChartType[] = ["histogram", "scatter", "line"];

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

export function ChartDetailPanel({ chartPath, name, document }: ChartDetailPanelProps) {
  const { t } = useTranslation();
  const { dataframes } = useEditorSessionResources();
  const databases = dataframes ?? {};

  const databaseOptions = useMemo(
    () =>
      Object.values(databases).map((db) => ({
        label: (db as { name?: string }).name ?? (db as { id: string }).id,
        value: (db as { id: string }).id,
      })),
    [databases],
  );

  const columns = useMemo(() => {
    const db = document.databaseId
      ? (databases[document.databaseId] as
          | { columns?: Array<{ name: string; type: string }> }
          | undefined)
      : undefined;
    return db?.columns ?? [];
  }, [document.databaseId, databases]);

  useEffect(() => {
    const databaseId = document.databaseId;
    if (!databaseId) return;
    const existing = databases[databaseId] as { columns?: unknown[] } | undefined;
    if (existing?.columns && existing.columns.length > 0) return;
    let cancelled = false;
    void hydrateDatabaseEditorMetadata(databaseId, () => cancelled);
    return () => {
      cancelled = true;
    };
  }, [document.databaseId, databases]);

  const numericColumns = columns.filter((c) => isNumericType(c.type));
  const allColumnOptions = columns.map((c) => ({ label: c.name, value: c.name }));
  const numericColumnOptions = numericColumns.map((c) => ({ label: c.name, value: c.name }));

  const patch = (changes: Parameters<typeof chartUi.updateDraft>[1]) => {
    chartUi.updateDraft(chartPath, changes);
  };

  const encodingLabelClass = "align-top pt-2";

  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField label={t("detail.fields.name")} tone="body">
          {name}
        </DetailReadonlyField>
        <DetailFieldRow label={t("chartsSidebar.dataset")} labelClassName={encodingLabelClass}>
          <Select
            value={document.databaseId}
            options={databaseOptions}
            onChange={(val) => patch({ databaseId: val, encodings: {} })}
          />
        </DetailFieldRow>
        <DetailFieldRow label={t("chartsSidebar.chartType")} labelClassName={encodingLabelClass}>
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
          <DetailFieldRow label={t("chartsSidebar.encodingY")} labelClassName={encodingLabelClass}>
            <Select
              value={document.encodings.y ?? ""}
              options={allColumnOptions}
              onChange={(val) => patch({ encodings: { ...document.encodings, y: val } })}
            />
          </DetailFieldRow>
        ) : (
          <>
            <DetailFieldRow
              label={t("chartsSidebar.encodingX")}
              labelClassName={encodingLabelClass}
            >
              <Select
                value={document.encodings.x ?? ""}
                options={numericColumnOptions}
                onChange={(val) => patch({ encodings: { ...document.encodings, x: val } })}
              />
            </DetailFieldRow>
            <DetailFieldRow
              label={t("chartsSidebar.encodingY")}
              labelClassName={encodingLabelClass}
            >
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
          <DetailColumnList columns={columns} emptyMessage={t("chartsSidebar.noColumns")} />
        </CardContent>
      </Card>
    </DetailPanelShell>
  );
}
