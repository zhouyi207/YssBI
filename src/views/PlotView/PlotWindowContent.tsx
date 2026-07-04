import type { ParsedPlotPayload } from '@/features/application/presentation';
import { useChartSeriesColors } from '@/shared/theme/chartTheme';
import Scatter, { type ScatterPoint } from '@/views/PlotView/Scatter';
import Line, { type LinePoint } from '@/views/PlotView/Line';
import ECDF, { type ECDFPoint } from '@/views/PlotView/ECDF';
import KDE, { type KDEPoint } from '@/views/PlotView/KDE';
import Histogram from '@/views/PlotView/Histogram';
import CorrelationPlot from '@/views/PlotView/CorrelationPlot';
import CorrelogramChart from '@/views/PlotView/CorrelogramChart';

interface PlotWindowContentProps {
  payload: ParsedPlotPayload | null;
  invalidFormatMessage: string;
  readyTitle: string;
  readyHint: string;
}

export function PlotWindowContent({
  payload,
  invalidFormatMessage,
  readyTitle,
  readyHint,
}: PlotWindowContentProps) {
  const seriesColors = useChartSeriesColors();

  if (!payload) {
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
        <span className="text-sm">{invalidFormatMessage}</span>
      </div>
    );
  }

  if (payload.kind === 'correlogram') {
    return (
      <div className="flex min-h-0 w-full flex-1 flex-col gap-2">
        <CorrelogramChart
          data={payload.data.acf}
          ciHalfWidth={payload.data.ci_half_width}
          title="ACF"
          valueLabel="ACF"
        />
        <CorrelogramChart
          data={payload.data.pacf}
          ciHalfWidth={payload.data.ci_half_width}
          title="PACF"
          color={seriesColors.secondary}
          valueLabel="PACF"
        />
      </div>
    );
  }

  if (payload.kind === 'histogram') {
    return (
      <div className="min-h-0 w-full flex-1">
        <Histogram
          data={payload.data.data}
          xLabel={payload.data.x_label}
          yLabel={payload.data.y_label}
        />
      </div>
    );
  }

  if (payload.kind === 'correlation') {
    return (
      <div className="min-h-0 w-full flex-1">
        <CorrelationPlot
          labels={payload.data.labels}
          matrix={payload.data.matrix}
          pMatrix={payload.data.p_matrix}
        />
      </div>
    );
  }

  const chart = payload.chart;
  const scatterData = payload.data;

  return (
    <div className="min-h-0 w-full flex-1">
      {chart === 'ecdf' ? (
        <ECDF
          data={scatterData.data as ECDFPoint[]}
          xLabel={scatterData.xLabel ?? scatterData.x_label}
          yLabel={scatterData.yLabel ?? scatterData.y_label}
        />
      ) : chart === 'kde' ? (
        <KDE
          data={scatterData.data as KDEPoint[]}
          xLabel={scatterData.xLabel ?? scatterData.x_label}
          yLabel={scatterData.yLabel ?? scatterData.y_label}
        />
      ) : chart === 'line' ? (
        <Line
          data={scatterData.data as LinePoint[]}
          xLabel={scatterData.xLabel ?? scatterData.x_label}
          yLabel={scatterData.yLabel ?? scatterData.y_label}
          xFormat={scatterData.xFormat}
          yFormat={scatterData.yFormat}
        />
      ) : chart === 'scatter' || chart === 'plot' ? (
        <Scatter
          data={scatterData.data as ScatterPoint[]}
          xLabel={scatterData.xLabel ?? scatterData.x_label}
          yLabel={scatterData.yLabel ?? scatterData.y_label}
          xFormat={scatterData.xFormat}
          yFormat={scatterData.yFormat}
        />
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center text-center text-muted-foreground">
          <h2 className="mb-2 text-2xl font-bold text-foreground">{readyTitle}</h2>
          <p>{readyHint}</p>
        </div>
      )}
    </div>
  );
}
