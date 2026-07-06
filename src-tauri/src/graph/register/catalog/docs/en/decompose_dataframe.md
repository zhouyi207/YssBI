# Decompose DataFrame

Split a **DataFrame** into one **DataSeries** output per column. Output pins are created dynamically from the input table schema when **DataFrame** is connected.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataFrame** | Input | Source table (typically from **Get DataFrame**) |
| *(column names)* | Output | One dynamic pin per column; type matches column dtype |

## Usage

Connect **DataFrame** first—column output pins appear automatically. Wire individual columns to transforms, comparisons, or **Combine DataFrame**. Each output pin name matches the source column name and carries the correct element type (`Float64`, `Int64`, `Boolean`, `Categorical`, etc.).
