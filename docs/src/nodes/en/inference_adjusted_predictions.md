# Adjusted predictions

Computes an adjusted outcome mean from a retained Linear, Logit or Probit fit.
Connect its **model** output. The estimation design and coefficient covariance
are reused; no new rows or response values are required.

## Parameters

- **evaluation**: average (default) averages predictions over the fitted sample;
  at_means predicts at the column means.
- **at**: optional assignments such as x1=2,x2=0. Use names from the fitted
  coefficient table. Each setting replaces that design column for every evaluated
  row. Unknown/ambiguous names and intercept overrides are rejected.
- **confidence_level**: default 0.95, strictly between 0 and 1.

Set all columns of a categorical coding or manually expanded interaction
consistently: the node does not regenerate design transformations.

## Calculation and output

With evaluated rows $x_i$, the average prediction and its Delta-method variance are

$$
\widehat m=\frac1N\sum_i h(x_i'\widehat\beta),\qquad
a=\frac1N\sum_i h'(x_i'\widehat\beta)x_i,\qquad
\operatorname{Var}(\widehat m)\approx a'\widehat V a.
$$

The result contains the adjusted mean, standard error, interval, family, sample
size and settings. This is uncertainty of the estimated mean, not an interval
for a new individual observation. Conventional linear covariance uses residual-df
t inference; robust linear covariance and binary fits use normal inference.
Binary probability intervals use the Delta method on the probability scale and
may extend beyond [0,1]; they are not silently clipped. Averaging probabilities
usually differs from predicting at the means.
