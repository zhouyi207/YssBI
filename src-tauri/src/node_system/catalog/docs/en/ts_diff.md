# TS Diff

Differences a **Value Series**:

$$
\Delta y_t = y_t - y_{t-\text{lag}}
$$

With an optional **Time Series**, matches Stata `D.`: differences only across adjacent **Interval** steps (no gaps). Without a time column, uses positional lag.

## Usage

Use on aligned series directly. For irregular times, connect **Time Series** and **Interval**, or run **TS Align** first.
