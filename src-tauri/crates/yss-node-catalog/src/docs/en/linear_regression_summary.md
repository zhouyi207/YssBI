# Linear Regression Summary

Connect **Model** from **Linear Regression** to **Model** here. OLS, WLS, and GLS share this node. Summary has no fit configuration and never re-estimates the model.

**Result** and **Report** share the upstream native result. The common report retains the actual estimation method, standard error method, and statistics. Coefficients, observations, and diagnostics are read on demand using existing result references.

WLS/GLS sums of squares and R² use their transformed scale; fitted values, residual plots, and the observation table use original units.
