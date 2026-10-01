# Time-dependent Cox regression

Supply `start`, `stop`, `event`, exact `subjects` identifiers and one or more numeric `predictors`. Each row is one interval with constant covariates. Require $0\le start<stop$, finite times, and event 0/1 (or false/true) at the interval's stop. Input row order is arbitrary. Subject identifiers can be numeric, text, categorical, ordinal or binary.

Intervals of one subject cannot overlap. Adjacent intervals are allowed; gaps represent time outside observation. Each subject may have at most one terminal event, on its last interval; no post-event intervals are accepted.

## Risk sets and model

$$ h_i(t)=h_0(t)\exp[x_i(t)^\top\beta],\qquad
R(t)=\{i:start_i<t\le stop_i\}.$$
The left-open convention prevents double-counting at adjoining interval boundaries. `survival_ties=efron` (default) or `breslow` controls tied events. No intercept is fitted. `max_iterations=500` (positive integer), `tolerance=0.0000001` (from 1e-12 to 0.01). Nonconvergence or a singular information matrix produces an error. Predictor columns must be finite and identifiable; categorical predictors require prior dummy coding.

`result` contains coefficients, hazard ratios, covariance, centered linear predictors per interval, Breslow baseline, partial log likelihood and iterations. `observations` counts intervals, not subjects; `events` counts terminal events. Baseline is centered at the interval-row covariate means. Coefficient tests use $H_0:\beta_j=0$ versus $H_1:\beta_j\ne0$, $z=\widehat\beta_j/SE$ and a standard normal reference; hazard-ratio intervals exponentiate the 95% coefficient intervals.

This counting-process formulation supports delayed entry and changing covariates under proportional hazards and independent censoring. It does not fit recurrent-event models or robust subject-cluster covariance. No future risk output or nomogram connection is offered because future covariate paths are not supplied.
$$
