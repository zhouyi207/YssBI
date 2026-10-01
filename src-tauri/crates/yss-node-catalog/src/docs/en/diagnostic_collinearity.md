# Collinearity diagnostics

Connect one or more aligned numeric **variables**. At least two observations are required. More variables than observations are allowed so rank deficiency can be diagnosed. Missing or nonfinite values fail.

**Include intercept** defaults to true: predictors are centered and an intercept is included. Disabling it uses the uncentered design. Design columns are scaled to unit length without changing their span. No response or fitted model is needed.

For the scaled design $Z$, the result reports rank, condition number and descending eigenvalues $\lambda_j$ of $Z^\mathsf{T}Z$. Condition indices are $\sqrt{\lambda_{\max}/\lambda_j}$. Centered VIF is $1/(1-R_j^2)$, with $R_j^2$ from regressing one predictor on the others. Disabling the intercept uses uncentered VIF.

**result** contains observations, columns, rank, full_column_rank, centered, condition_number, eigenvalues, condition_indices and terms (term, constant, vif, tolerance).

Intercept/zero-column VIF and tolerance are null. Perfectly dependent predictors have null VIF and zero tolerance. Unbounded condition numbers/indices are null, not finite sentinels. Near-zero eigenvalues follow numerical decomposition precision. This descriptive diagnostic has no p-value; high VIF alone does not mandate removing a variable.
