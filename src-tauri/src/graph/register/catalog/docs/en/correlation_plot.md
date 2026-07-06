# Correlation Plot

Computes Pearson correlation matrix and two-sided p-values from one or more numeric **DataSeries**, then draws a heatmap.

Use **+** on the node to add series; at least 2 connected **DataSeries** are required.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Control-flow entry |
| **DataSeries** | Input | Repeatable numeric series pins (≥2) |
| **Out** | Exec output | Control-flow exit |

## Usage

Running the graph opens the **Plot** window with the correlation heatmap. Rows align by position; pairwise null rows are dropped. Labels default to series names.
