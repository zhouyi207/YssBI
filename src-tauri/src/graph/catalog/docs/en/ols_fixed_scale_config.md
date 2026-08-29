# OLS Fixed Scale Config

Builds **OLSFixedScaleConfig** for `cov_type = 'fixed scale'`. Connect **Scale** (positive `Float64`) and wire **Config** to **OLS & WLS Configure** → **VCE**.

The sandwich uses a user-specified scale factor on the covariance matrix rather than estimating $\hat\sigma^2$ from residuals alone.

## Usage

1. Set **Scale** to the desired positive constant.
2. Connect **Config** output to **OLS & WLS Configure** → **VCE**.
3. Connect the resulting **OLSConfigure** to **OLS** / **WLS**.
