# TS Lag

Strict time-aligned lag (Stata `L.` semantics). Lags **Value Series** by **Lag** steps after aligning on **Time Series**.

When the time column is marked **Aligned** and matches the value length, re-alignment is skipped; otherwise **Interval** is inferred and values are aligned.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **Time Series** | Input | `Int64` or `Date` time series |
| **Value Series** | Input | `DataSeries<Float64>` |
| **Lag** | Input | Lag order; default 1 |
| **Time** | Output | Full aligned time grid |
| **Lagged** | Output | Lagged value series |

## Usage

First **Lag** observations are null. Duplicate time keys error; prefer **TS Align** for a regular grid first.
