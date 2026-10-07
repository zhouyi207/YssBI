# VEC Summary

Connect the `model` from VEC Fit. Configure selects model overview, coefficient inference and `cointegration` statistics by default. Enable `serial_tests` or `stability` to compute residual LM tests or stability roots from the fitted model. Residual LM tests use `serial_lags` (1–40, default 2) when enabled.

Outputs `result` containing the selected sections. No raw data or estimation parameters are required, and the model is not refitted.
