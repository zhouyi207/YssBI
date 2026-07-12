import React from 'react';

export const LazyFormulaBlock = React.lazy(() => import('../FormulaBlock'));
export const LazyFormulaBlock2SLS = React.lazy(() => import('../FormulaBlock2SLS'));
export const LazyBinaryFormulaBlock = React.lazy(() => import('../BinaryFormulaBlock'));
export const LazyVARFormulaBlock = React.lazy(() => import('../VARFormulaBlock'));
export const LazyPanelFormulaBlock = React.lazy(() => import('../PanelFormulaBlock'));
export const LazyResidualPlot = React.lazy(() => import('../ResidualPlot'));
export const LazyScatter = React.lazy(() => import('@/views/PlotView/Scatter'));
export const LazyKDE = React.lazy(() => import('@/views/PlotView/KDE'));
