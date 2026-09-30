import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ChartRenderer } from "@/shared/charts/ChartRenderer";
import type { ParsedPlotPayload } from "@/shared/types/dto/plotPayload";
import { LinePlotControls } from "./LinePlotControls";
import { toResultChartModel } from "./toResultChartModel";

export interface PlotResultViewProps {
  payload: ParsedPlotPayload | null;
  invalidContent: ReactNode;
}

function PlotInvalidState({ children }: { children: ReactNode }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground">
      <svg
        className="h-12 w-12 text-red-500/50"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
        aria-hidden
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z"
        />
      </svg>
      <div className="text-sm">{children}</div>
    </div>
  );
}

export function PlotResultView({ payload, invalidContent }: PlotResultViewProps) {
  const { t } = useTranslation();
  if (!payload) {
    return <PlotInvalidState>{invalidContent}</PlotInvalidState>;
  }

  const model = toResultChartModel(payload);
  const metadata = "metadata" in payload.data ? payload.data.metadata : undefined;
  const information: ReactNode[] = [];
  if (metadata)
    information.push(
      t(metadata.sampled ? "plot.sampled" : "plot.observations", {
        observations: metadata.observations,
        displayed: metadata.displayed,
      }),
    );
  if (payload.kind === "roc")
    information.push(
      `AUC = ${payload.data.auc.toFixed(4)}`,
      t("plot.rocCounts", { positives: payload.data.positives, negatives: payload.data.negatives }),
    );
  if (payload.kind === "coefficient")
    information.push(
      t("plot.confidence", { level: (payload.data.confidenceLevel * 100).toFixed(1) }),
    );
  if (payload.kind === "ppQq")
    information.push(
      t("plot.normalReference", {
        mean: payload.data.referenceMean.toPrecision(4),
        sd: payload.data.referenceStandardDeviation.toPrecision(4),
      }),
    );
  if (payload.kind === "quadrant")
    information.push(t("plot.quadrantCounts", { counts: payload.data.counts.join(" / ") }));
  if (payload.kind === "wordcloud")
    information.push(
      t("plot.wordCounts", {
        observations: payload.data.observations,
        displayed: payload.data.words.length,
        unique: payload.data.uniqueWords,
      }),
    );
  if (
    payload.kind === "histogram" &&
    "observations" in payload.data &&
    typeof payload.data.observations === "number"
  )
    information.push(t("plot.observations", { observations: payload.data.observations }));

  return (
    <div className="flex min-h-0 w-full flex-1 flex-col gap-2">
      {information.length > 0 && (
        <div className="flex shrink-0 flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
          {information.map((value, i) => (
            <span key={i}>{value}</span>
          ))}
        </div>
      )}
      {payload.kind === "line" && model.kind === "line" ? (
        <div className="min-h-0 w-full flex-1 overflow-hidden rounded-lg border border-border bg-card">
          <LinePlotControls model={model} />
        </div>
      ) : (
        <div className="flex min-h-0 w-full flex-1">
          <ChartRenderer model={model} />
        </div>
      )}
    </div>
  );
}
