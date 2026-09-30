# Repeated-measures ANOVA

Provide long-format numeric **Response**, **Subject** identifiers and 1–4 within-subject **Factor** series. At least two subjects are required. Each factor needs 2–32 observed categories; every subject must have exactly one observation in every factorial cell, up to 256 cells. Columns must align and relational series must share a row domain. Missing values, duplicate cells and incomplete designs are rejected without implicit aggregation or deletion. This node analyzes within-subject factors, excluding between-subject and mixed designs.

All main effects and interactions are tested. Each term's $H_0$ sets all corresponding within-subject contrast means to zero, versus at least one nonzero mean. Orthogonal contrasts separate effects from their subject-interaction errors:

$$F_T=\frac{SS_T/d_T}{SS_{S\times T}/[(s-1)d_T]}\sim F_{d_T,(s-1)d_T}.$$

$s$ is the subject count, $d_T$ the contrast dimension, and $SS_{S\times T}$ the positive subject-interaction error sum of squares. Uncorrected inference assumes independent subjects, approximately normal errors and sphericity for that effect.

**Sphericity correction** defaults to `greenhouse_geisser`; `none` is available. With $S_T$ the covariance of the term's orthogonal contrasts:

$$\epsilon_{GG}=\frac{[\operatorname{tr}(S_T)]^2}{d_T\operatorname{tr}(S_T^2)},\qquad df_1=\epsilon_{GG}d_T,\quad df_2=\epsilon_{GG}(s-1)d_T.$$

Correction preserves F and adjusts its reference degrees of freedom. The bound is $1/d_T\le\epsilon_{GG}\le1$; one-dimensional contrasts need no correction. Mauchly's test is not computed.

The single **Result** has a `table` with effect/error sums of squares, original degrees of freedom and mean squares, F, `p_value_uncorrected`, `epsilon_greenhouse_geisser`, selected `df_numerator`/`df_denominator`, `p_value` and partial $\eta^2$, plus subject count, cell count and original factor labels. With `none`, `p_value` is uncorrected. A small p-value supports a condition effect or interaction without post-hoc comparisons.

See [MathWorks sphericity and epsilon](https://www.mathworks.com/help/stats/compound-symmetry-assumption-and-epsilon-corrections.html) for correction definitions.
