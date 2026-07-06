# XT Align

Aligns a panel **DataFrame** on the full $(entity \times time)$ grid; missing cells become null.

Entity column: **Categorical**, **Int64**, or **String**. Time column: **Int64** or **Date**.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataFrame** | Input | Panel source table |
| **Entity Col** | Input | Entity ID column name |
| **Time Col** | Input | Time column name |
| **Interval** | Input | Optional time step |
| **Aligned** | Output | Balanced panel `DataFrame` |

## Usage

Standard step before panel models or **XT Diff**. Output keeps the input schema; rows expand to the entity–time Cartesian product.
