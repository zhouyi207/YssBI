import React, { Suspense, useMemo } from "react";
import { acfSeriesToBars, pacfSeriesToBars } from "@/shared/types/report";
import { useChartTheme } from "@/shared/charts/core";
import type { AcfPacfResult } from "@/shared/types/domain/resultReport";
import type { DeepReadonly } from "@/shared/types/deepReadonly";

const CorrelogramChart = React.lazy(() => import("@/shared/charts/statistical/CorrelogramChart"));

export function AcfPacfResultView({ result }: { result: DeepReadonly<AcfPacfResult> }) {
  const { series: seriesColors } = useChartTheme();
  const acf = useMemo(() => acfSeriesToBars(result.acf), [result.acf]);
  const pacf = useMemo(() => pacfSeriesToBars(result.pacf), [result.pacf]);
  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mt-4">
      <div>
        <Suspense fallback={<div className="h-[240px] animate-pulse bg-muted rounded" />}>
          <CorrelogramChart data={acf} ciHalfWidth={result.ciHalfWidth} title="ACF" />
        </Suspense>
      </div>
      <div>
        <Suspense fallback={<div className="h-[240px] animate-pulse bg-muted rounded" />}>
          <CorrelogramChart
            data={pacf}
            ciHalfWidth={result.ciHalfWidth}
            title="PACF"
            color={seriesColors.secondary}
          />
        </Suspense>
      </div>
    </div>
  );
}
