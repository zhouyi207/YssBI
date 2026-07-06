# VCE: HC3

HC3 (MacKinnon–White) uses a squared leverage adjustment; often preferred for finite-sample inference under heteroskedasticity.

## Formula

$$
\Omega_{ii} = \frac{\hat\varepsilon_i^2}{(1 - h_i)^2}
$$

## Output

| Pin | Description |
|-----|-------------|
| **VCE** | HC3 covariance constant handle |

## Usage

Connect **VCE** → **OLS & WLS Configure** → **VCE**, then to **OLS** / **WLS** / Summary nodes.
