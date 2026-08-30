# XT Align

Aligns a panel **DataFrame** on the full $(entity \times time)$ grid; missing cells become null.

Entity column: **Categorical**, **Int64**, or **String**. Time column: **Int64** or **Date**.

## Usage

Standard step before panel models or **XT Diff**. Output keeps the input schema; rows expand to the entity–time Cartesian product.
