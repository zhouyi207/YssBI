# VCE: Newey

Stata **`newey`**-style covariance (`cov_type = 'newey'`): Bartlett kernel with $n/(n-k)$ small-sample adjustment. Differs from **OLS HAC Config** (ivreg2 HAC implementation).

## Parameters

| Pin | Description |
|-----|-------------|
| **Lag** | optional maximum lag (`Int64`); default when unconnected |

Requires **Time** on **OLS & WLS Configure**. Wire **Config** → **VCE** on the configure node.
