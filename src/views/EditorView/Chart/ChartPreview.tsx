import { useTranslation } from "react-i18next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { useChartPreview } from "@/features/application/chart/useChartPreview";
import { toChartModel } from "@/features/application/chart/toChartModel";
import { ChartRenderer } from "@/shared/charts/ChartRenderer";
import type { ChartDocument, ChartPreviewPayload } from "@/shared/types/domain";
import { ChartEmptyState } from "./ChartEmptyState";

interface ChartPreviewProps {
  chartPath: string;
  document: ChartDocument | null;
}

type ChartPreviewErrorPayload = Extract<ChartPreviewPayload, { kind: "error" }>;

function ChartPreviewError({ error }: { error: ChartPreviewErrorPayload }) {
  const { t } = useTranslation();
  const summary =
    error.column === undefined
      ? t("chart.previewLoadFailed")
      : t("chart.previewColumnNotFound", { column: error.column });

  return (
    <div className="flex h-full items-center justify-center p-4">
      <Alert variant="destructive" className="max-w-md">
        <AlertTitle>{summary}</AlertTitle>
        <AlertDescription>
          <p>
            {t("common.errorCode")}: <code>{error.code}</code>
          </p>
          {error.incidentId ? (
            <p>
              {t("common.incidentId")}: <code>{error.incidentId}</code>
            </p>
          ) : null}
        </AlertDescription>
      </Alert>
    </div>
  );
}

function ChartPreviewContent({
  preview,
  loading,
}: {
  preview: ChartPreviewPayload;
  loading: boolean;
}) {
  if (preview.kind === "error") {
    return <ChartPreviewError error={preview} />;
  }
  if (preview.kind === "empty") {
    return loading ? null : (
      <div className="absolute inset-0 flex min-h-0">
        <ChartEmptyState />
      </div>
    );
  }

  const model = toChartModel(preview);
  if (!model) return null;

  return (
    <div data-chart-preview-region className="h-full w-full select-none">
      <ChartRenderer model={model} surface="plain" />
    </div>
  );
}

export function ChartPreview({ chartPath, document }: ChartPreviewProps) {
  const { preview, loading } = useChartPreview(chartPath, document);

  if (!document) {
    return <ChartEmptyState messageKey="chart.noActiveChart" />;
  }

  return (
    <div className="relative h-full w-full min-h-0 overflow-hidden bg-[var(--workbench-bg)]">
      {loading && preview.kind !== "empty" && (
        <div className="pointer-events-none absolute inset-0 z-10 bg-[var(--workbench-bg)]/40" />
      )}
      <ChartPreviewContent preview={preview} loading={loading} />
    </div>
  );
}
