# ECDF

Plots the empirical CDF $\hat F(x)$ from numeric **Values**: sorted observation $i$ maps to $y = i/n$.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Control-flow entry |
| **Values** | Input | `Float64` or `Int64` `DataSeries` |
| **Out** | Exec output | Control-flow exit |

## Usage

Running the graph opens the **Plot** window with the ECDF step curve. Null and non-finite values are dropped; at least one valid observation is required.
