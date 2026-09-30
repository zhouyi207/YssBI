# VAR Summary

Connect the fitted `model` from VAR Fit. Configure selects model overview and coefficient inference by default. Enable `lag_exclusion`, `serial_tests` or `stability` for the corresponding analyses. Residual LM tests use `serial_lags` (1–40, default 2) when enabled.

Outputs `result` containing the selected sections. Optional analyses are computed from the retained fit without refitting it. Use the separate Granger, IRF and FEVD nodes for those analyses.
