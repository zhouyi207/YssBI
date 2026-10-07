# IV LIML Summary

Connect the fitted `model` from IV LIML Fit. Configure selects model overview and coefficient inference by default. Enable `first_stage` or `overidentification` to compute those analyses from the same fitted sample and specification.

Outputs `result` containing the selected sections. No raw data or estimation parameters are required, and the LIML model is not refitted. Overidentification requires extra instruments and nonrobust covariance; unavailable statistics are reported as null.

Reports retain actual coefficient/instrument names and show structural and first-stage equations, named first-stage coefficients and weak-instrument critical values. Unavailable identification or covariance-dependent tests have explicit reasons. Optional `hypothesis_test`/`hypothesis` tests arbitrary independent coefficient restrictions: z/χ² by default, t/F with `small=true`, using the selected model covariance.
