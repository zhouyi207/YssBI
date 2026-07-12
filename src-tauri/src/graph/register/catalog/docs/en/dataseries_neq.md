# DataSeries Not Equal (!=)

Element-wise inequality: $\text{Result}_i = (\text{Series}_i \neq \text{Value}_i)$.

Supports numeric, boolean, and string operands. Output is a **Boolean** `DataSeries`.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataSeries** | Input | Left-hand series |
| **Value** | Input | Scalar (`Float64` / `Int64` / `Boolean` / `String`) or same-length **DataSeries** |
| **Result** | Output | Element-wise comparison, `DataSeries<Boolean>` |

## Usage

Connect **DataSeries** and **Value**. When both sides are **DataSeries**, lengths must match. Useful for flagging outliers or comparing against a constant threshold.
