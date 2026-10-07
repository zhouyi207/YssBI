# Panel Summary

Connect the fitted `model` from Panel Fit. Configure controls `model_summary`, `coefficient_table`, `effects_statistics` and `estimator_statistics`; all four are enabled by default. Estimator statistics follow the fitted method, including RE Wald or MLE likelihood statistics where applicable.

Outputs `result` containing the selected sections. Reuses the fitted coefficients, covariance and group statistics without refitting or requiring raw inputs.

Panel Fit additionally emits `fitted` and `residuals` on the estimator's actual scale: within-transformed FE, quasi-demeaned RE/MLE, first differences, between-group means, or original LSDV observations. The retained estimation sample includes its response, column-major design, coefficients and zero-based source-row groups. It never pretends a transformed residual is an original-scale prediction. The residual output connects to existing normality, ACF/PACF and serial-diagnostic nodes.

Panel estimation-scale Prediction accepts this model and already transformed predictor columns in estimation-design order (omit an automatically included intercept). It predicts on exactly that scale; it does not extrapolate absorbed effects to new entities.

Compare panel estimators accepts raw aligned response/predictors/entity/time with comma-separated `estimators` (maximum six, no duplicates). Each requested estimator uses the selected effects/intercept/covariance; incompatible or failed fits are explicitly reported alongside successful models, instead of discarding the full comparison. Coefficient rows carry original variable labels and explicit omitted-term reasons/categories.
