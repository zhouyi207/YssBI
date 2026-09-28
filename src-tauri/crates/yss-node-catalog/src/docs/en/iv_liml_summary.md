# IV LIML Summary

Connect the fitted `model` from IV LIML Fit. Configure selects model overview and coefficient inference by default. Enable `first_stage` or `overidentification` to compute those analyses from the same fitted sample and specification.

Outputs `result` and `report` containing the selected sections. No raw data or estimation parameters are required, and the LIML model is not refitted. Overidentification requires extra instruments and nonrobust covariance; unavailable statistics are reported as null.
