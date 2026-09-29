# Leverage

Connect an OLS or WLS `model` to obtain leverage for each fitted observation. There are no parameters; GLS is unsupported.

## Formula

$$
h_i=x_i'(X'X)^{-1}x_i\quad\text{(OLS)},
\qquad h_i=w_i x_i'(X'WX)^{-1}x_i\quad\text{(WLS)}.
$$

$X$ is the original design, $x_i'$ its row $i$, and $W$ the diagonal matrix of original precision weights. Leverage measures potential influence through predictor position, not residual outlyingness or actual influence.

## Outputs and usage

`result` and `report` are identical: `test=leverage` with an inner `result` array of $h_i$ in fitted-row order. No p-value or automatic observation removal is provided.

For full rank, leverage sums to the column count $k$ and averages $k/n$. Examine residuals alongside high leverage. Preserve the model's actual sample order when matching to the original table. Singular designs prevent calculation.

Method details: [Leverage diagnostics](https://www.statsmodels.org/dev/generated/statsmodels.stats.outliers_influence.OLSInfluence.html).
