import { useTranslation } from "react-i18next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { useWorksheetChartPreview } from "@/features/application/worksheet/useWorksheetChartPreview";
import { toWorksheetChartModel } from "@/features/application/worksheet/toWorksheetChartModel";
import { ChartRenderer } from "@/shared/charts/ChartRenderer";
import type { WorksheetDocument, WorksheetPreviewPayload } from "@/shared/types/domain";
import { WorksheetEmptyState } from "./WorksheetEmptyState";

interface WorksheetChartPreviewProps {
  worksheetPath: string;
  document: WorksheetDocument | null;
}

type WorksheetPreviewErrorPayload = Extract<WorksheetPreviewPayload, { kind: "error" }>;

function WorksheetPreviewError({ error }: { error: WorksheetPreviewErrorPayload }) {
  const { t } = useTranslation();
  const summary =
    error.column === undefined
      ? t("worksheet.previewLoadFailed")
      : t("worksheet.previewColumnNotFound", { column: error.column });

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

function WorksheetPreviewContent({
  preview,
  loading,
}: {
  preview: WorksheetPreviewPayload;
  loading: boolean;
}) {
  if (preview.kind === "error") {
    return <WorksheetPreviewError error={preview} />;
  }
  if (preview.kind === "empty") {
    return loading ? null : (
      <div className="absolute inset-0 flex min-h-0">
        <WorksheetEmptyState />
      </div>
    );
  }

  const model = toWorksheetChartModel(preview);
  if (!model) return null;

  return (
    <div data-worksheet-chart-region className="h-full w-full select-none">
      <ChartRenderer model={model} surface="plain" />
    </div>
  );
}

export function WorksheetChartPreview({ worksheetPath, document }: WorksheetChartPreviewProps) {
  const { preview, loading } = useWorksheetChartPreview(worksheetPath, document);

  if (!document) {
    return <WorksheetEmptyState messageKey="worksheet.noActiveWorksheet" />;
  }

  return (
    <div className="relative h-full w-full min-h-0 overflow-hidden bg-[var(--workbench-bg)]">
      {loading && preview.kind !== "empty" && (
        <div className="pointer-events-none absolute inset-0 z-10 bg-[var(--workbench-bg)]/40" />
      )}
      <WorksheetPreviewContent preview={preview} loading={loading} />
    </div>
  );
}
