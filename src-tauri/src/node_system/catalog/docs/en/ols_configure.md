# OLS & WLS Configure

Combines regression options into an **OLSConfigure** struct for **OLS**, **WLS**, or **OLS Summary** / **WLS Summary**.

## Typical VCE wiring

- **VCE: NonRobust** — classical $ \hat{\mathrm{Var}}(\hat\beta) = \hat\sigma^2 (X'X)^{-1} $
- **VCE: HC0–HC3** — heteroskedasticity-robust (White-type) estimators
- **OLS Cluster Config** — cluster-robust SEs by group ID
- **OLS HAC Config** — HAC / Newey–West style (ivreg2 kernel set)
- **VCE: Newey** — Stata `newey` style (Bartlett + $n/(n-k)$)
