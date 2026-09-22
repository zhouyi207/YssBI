import React from "react";

export const LazyEquation = React.lazy(() => import("@/components/ui-presentation/Equation"));
export const LazyFormulaBlock2SLS = React.lazy(() => import("../FormulaBlock2SLS"));
export const LazyBinaryFormulaBlock = React.lazy(() => import("../BinaryFormulaBlock"));
export const LazyVARFormulaBlock = React.lazy(() => import("../VARFormulaBlock"));
export const LazyPanelFormulaBlock = React.lazy(() => import("../PanelFormulaBlock"));
export const LazyResidualPlot = React.lazy(() => import("../ResidualPlot"));
export const LazyScatter = React.lazy(() =>
  import("@/shared/charts/cartesian/ScatterChart").then(({ ScatterChart }) => ({
    default: ScatterChart,
  })),
);
export const LazyKDE = React.lazy(() =>
  import("@/shared/charts/cartesian/KdeChart").then(({ KdeChart }) => ({ default: KdeChart })),
);
