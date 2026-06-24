import React, { useMemo, useState, useCallback } from 'react';
import Scatter from '@/views/PlotView/Scatter';

interface ResidualPlotProps {
  fitted: number[];
  residuals: number[];
  /** Leverage（帽子矩阵对角元），用于异常值高亮 */
  leverage?: number[];
  xLabel?: string;
  yLabel?: string;
}

const ResidualPlot: React.FC<ResidualPlotProps> = ({
  fitted,
  residuals,
  leverage,
  xLabel = 'Fitted Values',
  yLabel = 'Residuals',
}) => {
  const [outlierPct, setOutlierPct] = useState(5);

  const data = useMemo(
    () => fitted.map((f, i) => ({ x: f, y: residuals[i] })),
    [fitted, residuals],
  );

  const highlightIndices = useMemo(() => {
    if (!leverage || leverage.length !== data.length || outlierPct <= 0 || outlierPct >= 100)
      return undefined;
    const n = data.length;
    const k = Math.max(1, Math.ceil((n * outlierPct) / 100));
    const indicesWithLev = leverage.map((lev, i) => ({ i, lev }));
    indicesWithLev.sort((a, b) => b.lev - a.lev);
    return new Set(indicesWithLev.slice(0, k).map(({ i }) => i));
  }, [leverage, data.length, outlierPct]);

  const handlePctChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const v = e.target.valueAsNumber;
    if (Number.isFinite(v) && v >= 0 && v <= 100) setOutlierPct(v);
  }, []);

  return (
    <div className="w-full min-h-[280px]">
      {leverage && leverage.length === data.length && (
        <div className="flex items-center gap-2 mb-2 px-1">
          <span className="text-[11px] text-muted-foreground">异常值高亮:</span>
          <input
            type="number"
            min={0}
            max={100}
            value={outlierPct}
            onChange={handlePctChange}
            className="w-14 px-2 py-0.5 text-xs font-mono text-foreground bg-muted border border-border rounded focus:outline-none focus:border-[var(--accent-color)]"
          />
          <span className="text-[11px] text-muted-foreground">% (按 leverage 最高)</span>
        </div>
      )}
      <Scatter
        data={data}
        xLabel={xLabel}
        yLabel={yLabel}
        height={280}
        symmetricY
        zeroLine
        highlightIndices={highlightIndices}
        highlightColor="#ef4444"
      />
    </div>
  );
};

export default ResidualPlot;
