# Combine DataFrame

Assemble multiple **DataSeries** columns into a single **DataFrame**. Shorter series are padded with nulls to match the longest column length.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **Column** | Input (repeatable) | One or more **DataSeries** to stack as columns |
| **DataFrame** | Output | Combined table; column names come from series names (or `col_0`, `col_1`, …) |

## Usage

Connect at least one **Column** pin. Add more **Column** inputs as needed. Column names default to each series name; unnamed series become `col_i`. Use after **Decompose DataFrame** or manual series construction to rebuild a table for **Filter DataFrame** or econometric nodes.
