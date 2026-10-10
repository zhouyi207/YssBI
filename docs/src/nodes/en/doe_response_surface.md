# Response surface (full quadratic)

Connect aligned numeric **Y** and continuous **factors**, one column per factor.
Each factor is centered at the midpoint of its observed range and divided by its half-range.
OLS includes the intercept, all coded z terms, z² terms and pairwise zᵢzⱼ interactions.
Coefficients use coded coordinates; `centers` and `half_ranges` provide the conversion.

With p factors the model has `(p+1)(p+2)/2` parameters. Observations must exceed this count and the design must have full rank.
Constant/collinear factors and designs that cannot distinguish quadratic terms fail explicitly instead of silently dropping terms.

**result** contains coefficients, classical OLS standard errors and 95% intervals, covariance and fit statistics.
**details** contains coded Hessian eigenvalues and the stationary classification: maximum, minimum, saddle or degenerate.
For nondegenerate surfaces it also gives the stationary point in coded and original units, its predicted response and membership in the observed factor ranges.
A degenerate Hessian returns no unique stationary point.

**observations** retains every row with observation number, observed response, fitted response and residual in a paged relation, without a fixed row cap.

Errors are assumed independent and homoskedastic. Automatic lack-of-fit testing, constrained optimization and multiple-response optimization are not included.
Range membership checks the bounding box, not the convex hull or experimental confirmation. Assess model diagnostics and run confirmation trials before using a stationary point as an operating optimum.

[Reference: NIST quadratic response and stationary analysis](https://www.itl.nist.gov/div898/handbook/pri/section5/pri5314.htm)
