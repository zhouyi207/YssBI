# IV:2SLS Configure

Same options as **OLS & WLS Configure**, but defaults match Stata `ivregress 2sls` (no small-sample $n/(n-k)$ adjustment on VCE; Wald/z reporting).

## Inputs

| Pin | Description |
|-----|-------------|
| **Constant** | Include intercept |
| **VCE** | Any **VCE:** constant or OLS config node (**Cluster**, **HAC**, **Newey**, etc.) |
| **Time** | Time index; required when using HAC / Newey-style VCE |

## Output

| Pin | Description |
|-----|-------------|
| **Config** | **OLSConfigure** handle |

Wire **Config** to the optional **Config** pin on **IV:2SLS Summary** / **IV:LIML Summary**.
