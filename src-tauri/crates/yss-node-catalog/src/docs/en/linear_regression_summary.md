# Linear Regression Summary

Connect **Model** from **Linear Regression** to **Model** here. OLS, WLS, and GLS share this node. Summary has no fit configuration and never re-estimates the model.

Choose contents in **Parameters → Configure** on the Summary node. Model summary, coefficients, equation and ANOVA are selected by default. Optional contents include a coefficient chart, diagnostics, residual plot, observations, ACF/PACF, serial tests and a hypothesis test. Selecting an analysis exposes its lag, BG or hypothesis parameters.

**Result** and **Report** share the fitted model and retain this execution's selection and computed analyses. Only selected additional analyses and their dependencies run; matching analyses are reused for the same model and parameters. Fit computations remain necessary. Tables and plot projections are read on demand.

Use **Add contents → Add and compute** in Report to extend the same node configuration and execute its current Summary. The previous report remains readable until the updated result is ready. Changed model inputs produce a new result without mixing executions. Configuration supports undo/redo and explicit graph Save; adding contents does not implicitly save the graph.

WLS/GLS sums of squares and R² use their transformed scale; fitted values, residual plots, and the observation table use original units.
