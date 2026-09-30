import React, { Suspense } from "react";
import { acfSeriesToBars, pacfSeriesToBars } from "@/shared/types/report";
import { useChartTheme } from "@/shared/charts/core";

const CorrelogramChart = React.lazy(() => import("@/shared/charts/statistical/CorrelogramChart"));

export function AcfPacfResultView({
  result,
}: {
  result: { readonly acf: readonly number[]; readonly pacf: readonly number[]; readonly n: number };
}) {
  const { series: seriesColors } = useChartTheme();
  const ciHalfWidth = 1.96 / Math.sqrt(result.n);
  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mt-4">
      <div>
        <Suspense fallback={<div className="h-[240px] animate-pulse bg-muted rounded" />}>
          <CorrelogramChart
            data={acfSeriesToBars(result.acf)}
            ciHalfWidth={ciHalfWidth}
            title="ACF"
          />
        </Suspense>
      </div>
      <div>
        <Suspense fallback={<div className="h-[240px] animate-pulse bg-muted rounded" />}>
          <CorrelogramChart
            data={pacfSeriesToBars(result.pacf)}
            ciHalfWidth={ciHalfWidth}
            title="PACF"
            color={seriesColors.secondary}
          />
        </Suspense>
      </div>
    </div>
  );
}
