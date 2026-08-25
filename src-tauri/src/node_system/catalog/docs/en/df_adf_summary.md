# DF & ADF Summary

Runs DF/ADF over a grid of **Constant**, **Trend**, and **Lags** combinations on **Y**, returning a list result for side-by-side comparison.

Maximum lags follow Stata default: $\lfloor 12\,(T/100)^{1/4}\rfloor$. Sweeps $(constant,trend)\in\{(0,0),(1,0),(1,1)\}$ and $lags=0\ldots max\_lags$.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Control-flow entry |
| **Y** | Input | `DataSeries<Float64>` |
| **Result** | Output | `DFADFSummaryListResult` struct |
| **Out** | Exec output | Control-flow exit |

## Usage

Running publishes a summary report and list view in Info. Use **DF & ADF** for a single specification. Under-powered specs are skipped with a log message.
