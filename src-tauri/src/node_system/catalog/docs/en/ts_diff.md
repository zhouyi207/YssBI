# TS Diff

Differences a **Value Series**:

$$
\Delta y_t = y_t - y_{t-\text{lag}}
$$

With an optional **Time Series**, matches Stata `D.`: differences only across adjacent **Interval** steps (no gaps). Without a time column, uses positional lag.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **Value Series** | Input | `DataSeries<Float64>` |
| **Time Series** | Input | Optional; `Int64` or `Date` time column |
| **Lag** | Input | Lag order; default 1 |
| **Interval** | Input | Optional; time step when **Time Series** is connected |
| **Diff** | Output | Differenced series |

## Usage

Use on aligned series directly. For irregular times, connect **Time Series** and **Interval**, or run **TS Align** first.
