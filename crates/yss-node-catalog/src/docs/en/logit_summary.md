# Logit Summary

Consumes the fitted `model` produced by `yssbi.statistics.logit.fit`. Outputs `result` from that model. It does not accept raw data or estimation parameters and does not refit.

Enable `marginal_effects` to compute margins (off by default to keep ordinary summaries inexpensive). Evaluation defaults to `average` (AME); `at_means` uses covariate means (MEM). `marginal_at` accepts assignments such as `x1 = 0.75`; unassigned covariates retain observations or means. `dydx` is ∂p/∂x, `eydx` is ∂log(p)/∂x, `dyex` is x·∂p/∂x and `eyex` is x·∂log(p)/∂x. These are continuous-regressor derivatives, including for numeric 0/1 columns; they are not discrete factor contrasts. Standard errors use the analytic delta method on the final average; z tests and 95% intervals use the normal distribution. The intercept is excluded. Undefined ratios/zero-variance inference are not assigned invented values.

Classification defaults to cutoff 0.5, predicts 1 when p ≥ cutoff, and accepts cutoffs in [0,1]. The estimation-sample table reports TP/FP/FN/TN, sensitivity, specificity, PPV, NPV, accuracy and error rate; zero-denominator rates are unavailable. These are in-sample, not out-of-sample performance.

`hypothesis_test` enables named coefficient restrictions (`hypothesis`, default `x1 = 0`). A single equality uses a two-sided z test; an inequality uses its one-sided normal tail; multiple equalities use Wald χ² with degrees of freedom equal to the number of independent restrictions. Redundant restrictions fail. Use coefficient labels shown by the model.

[Reference: statsmodels discrete marginal effects](https://www.statsmodels.org/v0.14.6/generated/statsmodels.discrete.discrete_model.DiscreteResults.get_margeff.html).

Odds ratios are enabled by default for Logit only: OR = exp(β). The SE is exp(β)·SE(β); intervals exponentiate coefficient confidence limits. The z test retains H₀: β=0 (OR=1). Exponentiating the intercept gives baseline odds, explicitly labeled separately, not a per-unit covariate odds ratio.
