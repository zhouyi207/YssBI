import type { ParsedPlotPayload } from '@/features/application/presentation';
import { useChartTheme } from '@/shared/charts/core';
import Scatter from '@/views/PlotView/Scatter';
import Line from '@/views/PlotView/Line';
import ECDF from '@/views/PlotView/ECDF';
import KDE from '@/views/PlotView/KDE';
import Histogram from '@/views/PlotView/Histogram';
import CorrelationPlot from '@/views/PlotView/CorrelationPlot';
import CorrelogramChart from '@/views/PlotView/CorrelogramChart';

interface PlotWindowContentProps {
  payload: ParsedPlotPayload | null;
  invalidFormatMessage: string;
}

function PlotInvalidState({ message }: { message: string }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground">
      <svg className="h-12 w-12 text-red-500/50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z"
        />
      </svg>
      <span className="text-sm">{message}</span>
    </div>
  );
}

export function PlotWindowContent({
  payload,
  invalidFormatMessage,
}: PlotWindowContentProps) {
  const { series: seriesColors } = useChartTheme();

  if (!payload) {
    return <PlotInvalidState message={invalidFormatMessage} />;
  }

  switch (payload.kind) {
    case 'correlogram':
      return (
        <div className="flex min-h-0 w-full flex-1 flex-col gap-2">
          <CorrelogramChart
            data={payload.data.acf}
            ciHalfWidth={payload.data.ciHalfWidth}
            title="ACF"
            valueLabel="ACF"
          />
          <CorrelogramChart
            data={payload.data.pacf}
            ciHalfWidth={payload.data.ciHalfWidth}
            title="PACF"
            color={seriesColors.secondary}
            valueLabel="PACF"
          />
        </div>
      );

    case 'histogram':
      return (
        <div className="min-h-0 w-full flex-1">
          <Histogram
            data={payload.data.data}
            xLabel={payload.data.xLabel}
            yLabel={payload.data.yLabel}
          />
        </div>
      );

    case 'correlation':
      return (
        <div className="min-h-0 w-full flex-1">
          <CorrelationPlot
            labels={payload.data.labels}
            matrix={payload.data.matrix}
            pMatrix={payload.data.pMatrix}
          />
        </div>
      );

    case 'ecdf':
      return (
        <div className="min-h-0 w-full flex-1">
          <ECDF
            data={payload.data.data}
            xLabel={payload.data.xLabel}
            yLabel={payload.data.yLabel}
          />
        </div>
      );

    case 'kde':
      return (
        <div className="min-h-0 w-full flex-1">
          <KDE
            data={payload.data.data}
            xLabel={payload.data.xLabel}
            yLabel={payload.data.yLabel}
          />
        </div>
      );

    case 'line':
      return (
        <div className="min-h-0 w-full flex-1">
          <Line
            data={payload.data.data}
            xLabel={payload.data.xLabel}
            yLabel={payload.data.yLabel}
            xFormat={payload.data.xFormat}
            yFormat={payload.data.yFormat}
          />
        </div>
      );

    case 'scatter':
    case 'plot':
      return (
        <div className="min-h-0 w-full flex-1">
          <Scatter
            data={payload.data.data}
            xLabel={payload.data.xLabel}
            yLabel={payload.data.yLabel}
            xFormat={payload.data.xFormat}
            yFormat={payload.data.yFormat}
          />
        </div>
      );
  }
}
