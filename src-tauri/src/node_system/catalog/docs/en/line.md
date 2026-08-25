# Line

Line chart from **X** and **Y** `DataSeries`; pairs rows and skips null points.

**X** may be numeric or **Date**; **Y** must be plottable as numeric.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Control-flow entry |
| **X** | Input | `Float64` / `Int64` / `Date` `DataSeries` |
| **Y** | Input | Numeric `DataSeries` |
| **Out** | Exec output | Control-flow exit |

## Usage

Running the graph opens the **Plot** window; suited to time-series trajectories. **Date** axes use date formatting. Requires at least one valid $(x,y)$ pair.
