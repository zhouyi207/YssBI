# Leverage

Connect an OLS or WLS `model` to obtain leverage for every fitted observation. There are no parameters. Fitted observation order is preserved; GLS is unsupported.

## Calculation and meaning

Let $X$ be a full-column-rank $n\times k$ design, with row $x_i'$. The count $k$ includes any intercept. For OLS,

$$
H=X(X'X)^{-1}X',\qquad h_i=H_{ii}=x_i'(X'X)^{-1}x_i.
$$

WLS uses the original positive precision weights $W=\operatorname{diag}(w_i)$:

$$
H_w=W^{1/2}X(X'WX)^{-1}X'W^{1/2},
\qquad h_i=w_i x_i'(X'WX)^{-1}x_i.
$$

Under these conditions $0\leq h_i\leq1$, $\sum_i h_i=k$, and mean leverage is $k/n$. A large value indicates potential influence through predictor position, regardless of residual size. High leverage alone does not establish a response outlier or actual influence. See [statsmodels OLSInfluence](https://www.statsmodels.org/dev/generated/statsmodels.stats.outliers_influence.OLSInfluence.html).

## Outputs and interpretation

`result` and `report` are identical: `test=leverage` with an inner `result` array of $h_i$ in fitted-row order. There are no null/alternative hypotheses or p-values, and observations are not automatically flagged or removed.

For $n=100$, $k=5$, mean leverage is 0.05. Thresholds such as $2k/n=0.10$ or $3k/n=0.15$ are exploratory heuristics, not formal test critical values. Examine residuals, data quality and predictor ranges before assessing influence.

The design must be nonempty and full rank; WLS weights must be finite and positive. Singular matrices and nonfinite results fail. Rows refer to the model's actual sample, so match upstream filtering and ordering before joining values back to raw data.
