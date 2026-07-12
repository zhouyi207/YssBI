# OLS & WLS Configure

Combines regression options into an **OLSConfigure** struct for **OLS**, **WLS**, or **OLS Summary** / **WLS Summary**.

## Inputs

| Pin | Description |
|-----|-------------|
| **Constant** | Include intercept (default when unconnected follows node default) |
| **VCE** | Variance–covariance estimator — connect a **VCE:** constant or a config node (**Fixed Scale**, **Cluster**, **HAC**, **Newey**) |
| **Time** | Time index (`Int64` or `Date` `DataSeries`) when the chosen VCE requires ordering |

## Output

**Config** → wire to the optional **Config** pin on regression nodes.

## Typical VCE wiring

- **VCE: NonRobust** — classical $ \hat{\mathrm{Var}}(\hat\beta) = \hat\sigma^2 (X'X)^{-1} $
- **VCE: HC0–HC3** — heteroskedasticity-robust (White-type) estimators
- **OLS Cluster Config** — cluster-robust SEs by group ID
- **OLS HAC Config** — HAC / Newey–West style (ivreg2 kernel set)
- **VCE: Newey** — Stata `newey` style (Bartlett + $n/(n-k)$)
