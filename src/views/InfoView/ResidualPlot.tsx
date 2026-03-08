import React, { useMemo } from 'react';
import Scatter from '@/views/PlotView/Scatter';

interface ResidualPlotProps {
  fitted: number[];
  residuals: number[];
  xLabel?: string;
  yLabel?: string;
}

const ResidualPlot: React.FC<ResidualPlotProps> = ({
  fitted,
  residuals,
  xLabel = 'Fitted Values',
  yLabel = 'Residuals',
}) => {
  const data = useMemo(
    () => fitted.map((f, i) => ({ x: f, y: residuals[i] })),
    [fitted, residuals],
  );

  return (
    <div className="w-full min-h-[280px]">
      <Scatter
        data={data}
        xLabel={xLabel}
        yLabel={yLabel}
        height={280}
        symmetricY
        zeroLine
      />
    </div>
  );
};

export default ResidualPlot;
