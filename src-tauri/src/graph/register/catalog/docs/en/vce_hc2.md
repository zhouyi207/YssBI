# VCE: HC2

HC2 uses leverage-adjusted residual squares. Let $h_i = x_i'(X'X)^{-1}x_i$:

## Formula

$$
\Omega_{ii} = \frac{\hat\varepsilon_i^2}{1 - h_i}
$$

## Output

| Pin | Description |
|-----|-------------|
| **VCE** | HC2 covariance constant handle |

## Usage

Connect **VCE** → **OLS & WLS Configure** → **VCE**, then to **OLS** / **WLS** / Summary nodes.
