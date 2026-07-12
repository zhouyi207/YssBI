# DataSeries Less Equal (<=)

Element-wise less-or-equal: $\text{Result}_i = (\text{Series}_i \leq \text{Value}_i)$.

**Value** may be a numeric scalar or a same-length numeric **DataSeries**. Output is a **Boolean** `DataSeries`.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataSeries** | Input | Left-hand series |
| **Value** | Input | `Float64` / `Int64` scalar, or same-length numeric **DataSeries** |
| **Result** | Output | Element-wise comparison, `DataSeries<Boolean>` |

## Usage

Connect two equal-length numeric **DataSeries** for element-wise comparison, or a scalar threshold for broadcast comparison. `Boolean` / `String` types do not support `<=`.
