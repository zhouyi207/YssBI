# OLS HAC Config

Heteroskedasticity and autocorrelation consistent (HAC) covariance for `cov_type = 'HAC'` (ivreg2-style).

## Parameters

| Pin | Default | Options |
|-----|---------|---------|
| **Kernel** | Bartlett | Bartlett, Parzen, Quadratic Spectral |
| **Bandwidth** | automatic | optional lag / bandwidth (`Int64`) |

Requires a **Time** index on **OLS & WLS Configure** (or the regression node) when observations are time-ordered.

Wire **Config** → **OLS & WLS Configure** → **VCE**.
