# Linear Regression

Fit **Response** against ordered **Predictors**. Choose **OLS** (default), **WLS**, or **GLS** in model configuration, with an optional intercept.

- **OLS** requires the response and at least one predictor.
- **WLS** additionally requires one **Weights (WLS)** input. Supply aligned, finite, strictly positive precision weights proportional to inverse error variance, not survey sampling weights.
- **GLS** requires **Covariance column (GLS)** inputs in matrix column order: n observations require n columns of n finite values. The matrix must be symmetric positive definite and its row/column order must match the training sample. It is the relative error covariance Σ; the error variance is an estimated scale times Σ. Dense covariance uses O(n²) memory and is subject to the execution input budget.

Add only the auxiliary inputs required by the selected method; remove inapplicable inputs when switching methods. Response, predictors, and WLS weights must share a proven relational row domain. In-memory series are aligned by position and must have equal lengths. Missing and non-finite values are rejected; filter the common sample upstream.

Estimation and standard error methods are separate. OLS/WLS accept conventional, HC0–HC3, HAC, Newey–West, and fixed-scale errors. GLS currently accepts **nonrobust** only; other choices are rejected during execution.

**Model** contains the immutable fitted linear regression result, reusable by **Linear Regression Summary** and **Linear Prediction** in the same run. **Fitted** and **Residuals** remain in original observation units, not weighted or whitened units.
