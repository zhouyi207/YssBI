# Synthetic control

Builds a convex combination of untreated donor outcome series to match one treated unit before intervention.

## Inputs and parameters

Connect aligned numeric `Y` (treated unit) and one or more repeated `donors` series. Rows must already be in chronological order on a common, equally interpreted time grid; the node neither sorts nor aggregates them. Inputs must be complete and finite.

`pre_periods=10` uses exactly the first ten rows to fit weights. It must be a positive integer below the total row count; all remaining rows are post-treatment. `max_iterations=5000` is positive and `tolerance=0.0000001` lies in [1e-12,0.01]. The optimizer checks a simplex optimality gap on scaled pre-period data and fails explicitly if it does not converge.

## Estimator

For treated outcomes $y_t$ and donor vector $x_t$, solve

$$
\hat w=\arg\min_{w\ge0,\;\mathbf1'w=1}
\frac1{T_0}\sum_{t=1}^{T_0}(y_t-x_t'w)^2.
$$

There is no intercept, negative donor weight, covariate importance search or regularization penalty. Equal-weight initialization resolves numerically nonunique solutions deterministically. Post-treatment data do not enter weight fitting.

The synthetic outcome is $\hat y_t^0=x_t'\hat w$, with gap $g_t=y_t-\hat y_t^0$. The reported post effect is the mean post-treatment gap; pre/post RMSPE is the square root of the mean squared gap within each period block.

## Result and interpretation

`result` contains `donor_weights` in input donor order, full-row `synthetic` and `gaps`, `post_effect`, `pre_rmspe`, `post_rmspe`, period counts and iterations. One donor is allowed and receives weight one.

Causal interpretation requires suitable untreated donors, no interference/anticipation, and a credible counterfactual relationship after intervention. Poor pre-treatment fit weakens the comparison. This node does not produce placebo p-values, conventional confidence intervals or an independent-sample ATE. Method background: [synthetic controls and balancing](https://www.nber.org/papers/w22791).
