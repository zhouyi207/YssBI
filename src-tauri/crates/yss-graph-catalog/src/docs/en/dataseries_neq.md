# DataSeries Not Equal (!=)

Element-wise inequality: $\text{Result}_i = (\text{Series}_i \neq \text{Value}_i)$.

Supports numeric, boolean, and string operands. Output is a **Boolean** `DataSeries`.

## Usage

Connect **DataSeries** and **Value**. When both sides are **DataSeries**, lengths must match. Useful for flagging outliers or comparing against a constant threshold.
