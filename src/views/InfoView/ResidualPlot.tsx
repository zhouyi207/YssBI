import React, { useMemo } from 'react';
import Scatter from '@/views/PlotView/Scatter';

const ResidualPlot: React.FC<{ fitted: number[]; residuals: number[] }> = ({ fitted, residuals }) => {
  const data = useMemo(
    () => fitted.map((f, i) => ({ x: f, y: residuals[i] })),
    [fitted, residuals],
  );

  return (
    <div className="w-full min-h-[280px]">
      <Scatter
        data={data}
        xLabel="Fitted Values"
        yLabel="Residuals"
        height={280}
        symmetricY
        zeroLine
      />
    </div>
  );
};

export default ResidualPlot;
