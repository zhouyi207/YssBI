# Linear Prediction

Connect **Model** from **Linear Regression**, then connect new **Predictors** in training-column order. OLS, WLS, and GLS reuse the fitted coefficients and intercept without fitting again. No training response, weights, or covariance matrix is required.

The predictor count must match the fitted model; new columns must be aligned and finite. **Prediction** outputs point predictions in original units.
