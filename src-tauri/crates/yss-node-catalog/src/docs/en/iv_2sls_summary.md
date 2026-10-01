# IV 2SLS Summary

Connect the fitted `model` from IV 2SLS Fit. Configure selects model overview and coefficient inference by default. Enable `first_stage`, `overidentification` or `endogeneity` for additional analyses, computed only when selected using the same fitted sample and specification.

Outputs `result` containing the selected sections. No raw data or estimation parameters are required, and the IV model is not refitted. Overidentification requires extra instruments; the current endogeneity tests require nonrobust covariance. Unavailable statistics are reported as null.

Reports retain actual coefficient/instrument names and show structural and first-stage equations, named first-stage coefficients and weak-instrument critical values. Unavailable identification or covariance-dependent tests have explicit reasons. Optional `hypothesis_test`/`hypothesis` tests arbitrary independent coefficient restrictions: z/χ² by default, t/F with `small=true`, using the selected model covariance.
