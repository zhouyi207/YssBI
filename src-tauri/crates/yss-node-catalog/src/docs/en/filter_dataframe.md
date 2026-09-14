# Filter DataFrame

Keep rows of a **DataFrame** where a boolean **Condition** **DataSeries** is true. Output schema matches the input table.

## Usage

Build **Condition** with comparison nodes (e.g. **DataSeries** compare) or logical ops so it has one boolean value per row. Connect **DataFrame** and **Condition**, then run the graph. Rows where the mask is false are dropped; null or mismatched lengths raise an error.
