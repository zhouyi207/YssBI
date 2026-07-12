# TS Rolling Mean

Computes a rolling mean over **Window** observations; first **Window − 1** values are null.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataSeries** | Input | `DataSeries<Float64>` |
| **Window** | Input | Window length; must be a positive integer |
| **Rolling Mean** | Output | Rolling-mean series |

## Usage

Use for smoothing or moving averages. **Window** must be positive; nulls inside the window follow Polars rolling rules.
