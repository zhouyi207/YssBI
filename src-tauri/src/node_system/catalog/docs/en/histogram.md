# Histogram

Plots a histogram from numeric **Values** (`Float64` or `Int64` `DataSeries`).

Bin count uses Sturges' rule: $k = \lceil \log_2 n + 1 \rceil$ (capped at 100). Null and non-finite values are dropped before plotting.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Control-flow entry |
| **Values** | Input | Numeric `DataSeries` |
| **Out** | Exec output | Control-flow exit |

## Usage

Running the graph opens the **Plot** window with the histogram; repeated runs refresh the chart. Requires at least one valid value.
