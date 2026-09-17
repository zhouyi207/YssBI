# Decompose DataFrame

Split a **DataFrame** into one **DataSeries** output per column. Output pins are created dynamically from the input table schema when **DataFrame** is connected.

## Usage

Connect **DataFrame** first—column output pins appear automatically. Wire individual columns to transforms, comparisons, or **Assemble DataFrame**. Each output pin name matches the source column name and carries its resolved semantic element type.
