import { memo } from "react";
import { useTranslation } from "react-i18next";
import { useDescriptionResult } from "@/features/application/results/useDescriptionResult";
import type { DescriptionColumn } from "@/features/application/results/descriptionResult";
import { ResultReadError } from "@/features/application/results/components/ResultReadError";
import { formatNum } from "@/shared/stats/formatStat";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";
import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";
import { DetailText } from "../shared/DetailText";

const COMMON_FIELDS = ["semantic", "count", "missing"] as const;
const NUMERIC_FIELDS = ["mean", "std", "min", "q25", "median", "q75", "max"] as const;
const CATEGORY_FIELDS = ["unique"] as const;

function displayValue(value: string | number | boolean | null) {
  if (value === null) return "—";
  return typeof value === "number" && !Number.isInteger(value)
    ? formatNum(value)
    : displayCode(value);
}

function displayCode(value: string | number | boolean) {
  return value === "" ? '""' : String(value);
}

function ColumnSummary({ column }: { column: DescriptionColumn }) {
  const { t } = useTranslation();
  const fields = [
    ...COMMON_FIELDS.map((key) => [key, column[key]] as const),
    ...(column.semantic === "Numeric"
      ? NUMERIC_FIELDS.map((key) => [key, column[key]] as const)
      : CATEGORY_FIELDS.map((key) => [key, column[key]] as const)),
  ];
  return (
    <DetailCollapsibleSection title={column.column}>
      <DetailForm>
        {fields.map(([key, value]) => (
          <DetailReadonlyField key={key} label={t(`detail.description.fields.${key}`)}>
            {displayValue(value)}
          </DetailReadonlyField>
        ))}
      </DetailForm>
      {column.semantic !== "Numeric" && (
        <DetailCollapsibleSection title={t("detail.description.categories")}>
          {column.categories.length === 0 ? (
            <DetailText tone="muted">{t("detail.description.noCategories")}</DetailText>
          ) : (
            column.categories.map((category) => (
              <DetailCollapsibleSection key={category.position} title={displayCode(category.value)}>
                <DetailForm>
                  <DetailReadonlyField label={t("detail.description.fields.value")}>
                    {displayCode(category.value)}
                  </DetailReadonlyField>
                  {category.label !== undefined && (
                    <DetailReadonlyField label={t("detail.description.fields.label")}>
                      {displayValue(category.label)}
                    </DetailReadonlyField>
                  )}
                  <DetailReadonlyField label={t("detail.description.fields.frequency")}>
                    {displayValue(category.frequency)}
                  </DetailReadonlyField>
                  <DetailReadonlyField label={t("detail.description.fields.proportion")}>
                    {displayValue(category.proportion)}
                  </DetailReadonlyField>
                </DetailForm>
              </DetailCollapsibleSection>
            ))
          )}
        </DetailCollapsibleSection>
      )}
    </DetailCollapsibleSection>
  );
}

const DescriptionResultContent = memo(function DescriptionResultContent({
  graphPath,
  nodeId,
}: {
  graphPath: string;
  nodeId: string;
}) {
  const { t } = useTranslation();
  const result = useDescriptionResult(graphPath, nodeId);
  if (!result.available)
    return <DetailText tone="muted">{t("detail.description.unavailable")}</DetailText>;
  if (result.error)
    return <ResultReadError error={result.error} onRetry={() => void result.reload()} />;
  if (result.invalid)
    return <DetailText tone="muted">{t("detail.description.invalid")}</DetailText>;
  if (result.loading) return <DetailText tone="muted">{t("common.loading")}</DetailText>;
  return (
    <div className="min-w-0 space-y-1">
      {result.columns?.map((column) => (
        <ColumnSummary key={column.column} column={column} />
      ))}
    </div>
  );
});

export function DescriptionResultSection({
  graphPath,
  nodeId,
}: {
  graphPath: string;
  nodeId: string;
}) {
  const { t } = useTranslation();
  return (
    <DetailCollapsibleSection title={t("detail.description.result")}>
      <DescriptionResultContent
        key={`${graphPath}:${nodeId}`}
        graphPath={graphPath}
        nodeId={nodeId}
      />
    </DetailCollapsibleSection>
  );
}
