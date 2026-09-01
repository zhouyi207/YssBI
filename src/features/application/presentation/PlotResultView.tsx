import type { ReactNode } from "react";
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
  if (!payload) {
    return <PlotInvalidState>{invalidContent}</PlotInvalidState>;
  }

  const model = toResultChartModel(payload);

  return (
    <div className="flex min-h-0 w-full flex-1 flex-col">
      {model.kind === "line" ? (
        <div className="min-h-0 w-full flex-1 overflow-hidden rounded-lg border border-border bg-card">
          <LinePlotControls model={model} />
        </div>
      ) : (
        <ChartRenderer model={model} />
      )}
    </div>
  );
}
