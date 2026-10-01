# Prais?Winsten Summary

Consumes the fitted `model` produced by `yssbi.statistics.prais.fit`. Outputs `result` from that model. It does not accept raw data or estimation parameters and does not refit.

The report includes the AR(1) error equation u[t]=ρu[t−1]+ε[t], the actual transformation and the full-precision rho history for every iteration. Rho is an estimated serial-correlation parameter; the report does not invent its standard error. Optional coefficient restrictions use t/F inference with the fitted residual degrees of freedom, including the lost first observation under Cochrane–Orcutt.
