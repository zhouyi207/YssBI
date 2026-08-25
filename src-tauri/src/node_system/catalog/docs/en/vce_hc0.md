# VCE: HC0

White heteroskedasticity-robust covariance (HC0, no small-sample correction).

## Formula

$$
\widehat{\mathrm{Var}}(\hat\beta) = (X'X)^{-1} X' \Omega X (X'X)^{-1}, \quad \Omega = \mathrm{diag}(\hat\varepsilon_i^2)
$$

## Output

| Pin | Description |
|-----|-------------|
| **VCE** | HC0 covariance constant handle |

## Usage

Connect **VCE** → **OLS & WLS Configure** → **VCE**, then to **OLS** / **WLS** / Summary nodes.
