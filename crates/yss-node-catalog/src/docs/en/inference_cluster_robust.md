# Cluster-robust standard errors

Fits ordinary least squares with one-way clustered CR1 sandwich covariance.
Observations within a cluster may be dependent; clusters are assumed independent.

## Inputs

- **Y**: finite numeric response.
- **clusters**: one non-null numeric, text or categorical cluster identifier per row.
- **X₁, X₂, …**: one or more finite numeric columns.

All inputs must have equal lengths and pair by current position, including mixed database and in-memory inputs. Missing values are rejected.
There is no fixed row limit. At least two clusters, a full-rank design and more
observations than parameters are required.

## Parameter

**constant** includes an intercept by default.

## Calculation and output

For $G$ clusters, $N$ observations and $K$ parameters, the covariance is

$$
\widehat V=\frac{G}{G-1}\frac{N-1}{N-K}
(X'X)^{-1}\left(\sum_g X_g'\widehat u_g\widehat u_g'X_g\right)(X'X)^{-1}.
$$

The result contains coefficients, covariance, cluster and observation counts,
the correction factor, and coefficient t tests and 95% intervals with $G-1$
degrees of freedom. Fitted coefficients agree with ordinary OLS; their uncertainty
changes. With few clusters, this asymptotic approximation can be inaccurate.
This is one-way clustering, not multiway clustering or a wild cluster bootstrap.
