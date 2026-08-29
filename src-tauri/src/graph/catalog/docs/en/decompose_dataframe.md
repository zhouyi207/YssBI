# Decompose DataFrame

Split a **DataFrame** into one **DataSeries** output per column. Output pins are created dynamically from the input table schema when **DataFrame** is connected.

## Usage

Connect **DataFrame** first—column output pins appear automatically. Wire individual columns to transforms, comparisons, or **Combine DataFrame**. Each output pin name matches the source column name and carries the correct element type (`Float64`, `Int64`, `Boolean`, `Categorical`, etc.).
