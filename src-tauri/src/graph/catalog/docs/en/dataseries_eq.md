# DataSeries Equal (==)

Element-wise equality: $\text{Result}_i = (\text{Series}_i = \text{Value}_i)$.

Supports numeric, boolean, and string operands. Output is a **Boolean** `DataSeries`.

## Usage

Connect **DataSeries** and **Value**. When both sides are **DataSeries**, lengths must match or the node errors. Scalar values are broadcast to every row. Pipe **Result** into **Filter DataFrame** or logic nodes.
