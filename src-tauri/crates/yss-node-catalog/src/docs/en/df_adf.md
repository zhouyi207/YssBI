# DF & ADF

Input `series` is a numeric series in time order. Parameters `lags` and `regression` select the lag order and deterministic terms. Outputs `result` (`statistics.result.adf`) and `report` from the same test; no fitted model is produced. The result contract covers the test statistic, p-value, critical values, used lags, effective sample size and specification.

The null hypothesis is that the series has a unit root. This node has a catalog contract; its execution kernel is not yet registered.
