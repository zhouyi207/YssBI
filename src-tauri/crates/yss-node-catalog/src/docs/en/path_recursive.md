# Path analysis (observed recursive linear models)

Connect at least two continuous variables in port order, referenced as x1, x2, … . `equations` defaults to `x2 ~ x1; x3 ~ x1 + x2`, requiring three inputs for this example. Separate equations with semicolons or newlines. Each response has one equation and predictors are separated by plus signs. Every equation includes an intercept. References are independent of column names.

Only directed acyclic models are supported. Self edges, cycles, repeated predictors/responses and out-of-range references fail. Latent variables, correlated errors, equality constraints and full lavaan syntax are not supported.

Each equation is fitted by OLS. All columns must be aligned, finite and complete, without automatic row deletion. Each design must have full rank and n greater than its parameter count. Equations need not be written in topological order.

**result** reports input names, equation indices, original-unit coefficients, classical homoskedastic covariance and per-equation fit statistics. Coefficients use two-sided zero-coefficient t tests with n−k degrees of freedom and 95% intervals. This is equation-wise OLS and does not report global latent-variable SEM chi-square fit, CFI, TLI or RMSEA.

**effects** contains all ordered pairs of distinct variables: `source, target, direct, indirect, total, standardized_total`, using 1-based indices. Direct effects are edge coefficients. Total effects sum coefficient products along all directed paths; indirect effects subtract direct from total. Pairs with no path have zero effects. Standardized totals multiply total by source sample SD divided by target sample SD; zero variance yields an absent value. Composite effects are point estimates without uncomputed standard errors or p values.

**observations** retains every observation for every equation: `variable, observation, response, fitted, residual`, ordered first by written equation order, then by source row. There is no fixed observation/variable ceiling; execution resource admission accounts for result size.

Interpretation assumes independent equation errors, appropriate exogenous variables and a correct linear structure. Decomposition describes the specified model and does not alone establish causality.

[Reference: lavaan observed-variable mediation and path decomposition](https://lavaan.ugent.be/tutorial/mediation.html)
