# DataSeries Less Than (<)

Element-wise less-than: $\text{Result}_i = (\text{Series}_i < \text{Value}_i)$.

**Value** may be a numeric scalar or a same-length numeric **DataSeries**. Output is a **Boolean** `DataSeries`.

## Usage

Connect two equal-length numeric **DataSeries** for element-wise comparison, or a scalar threshold for broadcast comparison. `Boolean` / `String` types do not support `<`.
