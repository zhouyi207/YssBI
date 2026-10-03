# Moderated mediation (one moderated stage)

Connect response Y, predictor X, mediator M and moderator W, with optional continuous covariates. All explanatory columns use observed sample mean centering; `input_centers` follows X, M, W, covariates. Both equations contain an intercept, W main effect and all covariates.

`stage=first` (default) adds Xc×Wc to the mediator equation, with X coefficient a and interaction aW. The outcome equation is Y~Xc+Mc+Wc+covariates. The conditional indirect effect is `(a+aW×Wc)×b`; the moderated-mediation index is aW×b, where b is the outcome equation's M coefficient.

`stage=second` adds Mc×Wc to the outcome equation, with interaction coefficient bW; the mediator equation is M~Xc+Wc+covariates. The conditional indirect effect is `a×(b+bW×Wc)` and the index is a×bW. The direct effect c′ is constant in W; conditional total effects add c′ to conditional indirect effects.

`probe_sd` defaults to 1 and must be positive. Probe W at its mean and mean ± this multiple of its sample SD. Reported W values use original units and `in_observed_range` identifies extrapolation. `replications` defaults to 1000; zero disables effect inference, otherwise at least two are required. `seed` defaults to 42 and must be nonnegative.

**result** includes OLS coefficients, classical homoskedastic covariance and fit statistics for both equations; `details` contains the direct effect, three conditional indirect/total effects and `moderated_mediation_index`. Equation zero-coefficient tests are two-sided t tests with n−k degrees of freedom and 95% intervals. Effect/index standard errors and 95% intervals use an iid paired-row bootstrap and Type-7 2.5th/97.5th percentiles. No effect p values are reported. An index interval excluding zero is interval evidence of linear change in the indirect effect across W. Probe intervals have no multiplicity adjustment.

Both equations use the same sampled rows in every replication and fixed observed centers/probe coordinates. Failed fits abort, never silently disappear. **observations** retains every row and both equations' responses, fits and residuals: `observation, response, fitted, residual, mediator, mediator_fitted, mediator_residual`.

Inputs must be aligned finite complete numeric columns; designs require full rank and n>k. No fixed row ceiling. Simultaneous moderation of both stages, direct-path moderation, multiple/latent mediators and cluster inference are outside this model. Associations alone do not establish causal mechanisms.
