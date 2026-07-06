# VCE: NonRobust

Classical OLS variance under homoskedasticity and no autocorrelation.

## Formula

$$
\widehat{\mathrm{Var}}(\hat\beta) = \hat\sigma^2 (X'X)^{-1}, \quad \hat\sigma^2 = \frac{RSS}{n-k}
$$

## Output

| Pin | Description |
|-----|-------------|
| **VCE** | Classical covariance constant handle |

## Usage

Connect **VCE** → **OLS & WLS Configure** → **VCE**, then to **OLS** / **WLS** / Summary nodes.
