# IV 2SLS Summary

Connect the fitted `model` from IV 2SLS Fit. Configure selects model overview and coefficient inference by default. Enable `first_stage`, `overidentification` or `endogeneity` for additional analyses, computed only when selected using the same fitted sample and specification.

Outputs `result` containing the selected sections. No raw data or estimation parameters are required, and the IV model is not refitted. Overidentification requires extra instruments; the current endogeneity tests require nonrobust covariance. Unavailable statistics are reported as null.
