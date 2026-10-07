import { lazy, memo, Suspense } from "react";
import type { ChartModel } from "@/shared/charts/ChartModel";
import type { Coefficient } from "@/shared/types/report";
import { CoeffBarChart } from "./CoefficientChart";

const ChartRenderer = lazy(() =>
  import("@/shared/charts/ChartRenderer").then((module) => ({ default: module.ChartRenderer })),
);

export const Chart = memo(function Chart(
  props: { model: ChartModel } | { coefficients: Coefficient[] },
) {
  if ("coefficients" in props) return <CoeffBarChart coefficients={props.coefficients} />;
  return (
    <Suspense fallback={<div className="h-[280px] animate-pulse rounded-lg bg-muted" />}>
      <div className="h-[280px] min-w-0">
        <ChartRenderer model={props.model} />
      </div>
    </Suspense>
  );
});
