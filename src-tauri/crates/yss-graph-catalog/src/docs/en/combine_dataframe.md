# Combine DataFrame

Assemble multiple **DataSeries** columns into a single **DataFrame**. Shorter series are padded with nulls to match the longest column length.

## Usage

Connect at least one **Column** pin. Add more **Column** inputs as needed. Column names default to each series name; unnamed series become `col_i`. Use after **Decompose DataFrame** or manual series construction to rebuild a table for **Filter DataFrame** or econometric nodes.
