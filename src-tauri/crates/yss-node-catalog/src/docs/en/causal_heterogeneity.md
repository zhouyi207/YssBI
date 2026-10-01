# Treatment-effect heterogeneity

Tests equality of treatment effects across observed groups, with optional additive covariate adjustment.

## Inputs

Connect aligned numeric `response`, binary or numeric 0/1 `treatment`, one categorical `groups` series, and optional numeric `predictors`.
Boolean false/true correspond to control/treated. Groups accept numeric codes, binary, categorical, ordinal, text or identifier values. Their original labels are preserved in first-appearance order. At least two groups and both treatment levels within every group are required. Missing values, rank deficiency and leverage-one designs are rejected. There are no method parameters.

## Model and test

The first group is the reference. The model includes an intercept, covariates, treatment, group indicators and treatment-by-group interactions:

$$
Y_i=\alpha+X_i'\beta+\tau D_i+
\sum_{g=2}^G\gamma_g1[G_i=g]+
\sum_{g=2}^G\delta_gD_i1[G_i=g]+\epsilon_i.
$$

$H_0:\delta_2=\cdots=\delta_G=0$ states that all conditional group treatment effects are equal; $H_1$ allows at least one difference. The HC3 Wald statistic

$$
W=\hat\delta'\widehat{\operatorname{Var}}(\hat\delta)^{-1}\hat\delta
$$

has asymptotic $\chi^2_{G-1}$ reference distribution under the null. The full model needs positive residual degrees of freedom and the interaction covariance must be invertible. Small p-values provide evidence of group differences.

Group effects are $\tau$ for the reference and $\tau+\delta_g$ otherwise. Their individual zero-effect tests and coefficient tests use normal Wald statistics, two-sided p-values and 95% intervals. This is an observed-group interaction test, not Meta-analysis Cochran Q. A causal reading requires an appropriate treatment identification design; the test alone does not remove confounding.

## Result

`result` retains original `groups`, ordered `group_effects`, `equality_test`, full model `coefficients` and HC3 `covariance`.
Group references in coefficient names are one-based positions in the group list.
