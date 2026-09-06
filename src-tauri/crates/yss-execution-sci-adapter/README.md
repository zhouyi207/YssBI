# yss-execution-sci-adapter

`yss-sci-runtime` implementation of Execution's live scientific backend port.

The adapter owns ACF/PACF and OLS request, result, control, and typed-error mapping between
`yss-execution` and `yss-sci-runtime`. It has no dependency on Tauri, Application, Project, or
Database state.

OLS accepts materialized finite response/predictor columns, an explicit intercept flag, and a typed
covariance choice (nonrobust, HC0–HC3, fixed scale, HAC, or Newey-West). It validates configuration,
calls the SCI OLS API, and returns coefficients, fitted values, residuals, and a report. It does not
resolve data handles or read project settings; these are outside the backend port.
