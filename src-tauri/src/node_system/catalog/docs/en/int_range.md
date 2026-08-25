# Int Range

Builds an **Int64** `DataSeries` of consecutive integers:

$$
\text{start},\ \text{start}+1,\ \ldots,\ \text{start}+\text{length}-1
$$

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **Start** | Input | First value (`Int64`) |
| **Length** | Input | Number of elements; must be non-negative |
| **Col Name** | Input | Series name; defaults to `id` when empty |
| **DataSeries** | Output | `DataSeries<Int64>` |

## Usage

Useful for row indices, counters, or panel IDs. **Length** 0 yields an empty series.
