# Pareto

The Pareto distribution $\mathrm{Pareto}(\alpha, x_m)$ models power-law tails for $x \ge x_m$:

$$
f(x)=\frac{\alpha x_m^\alpha}{x^{\alpha+1}},\quad x \ge x_m
$$

## Pins

| Pin | Description |
|-----|-------------|
| **Shape** | shape parameter $\alpha > 0$ |
| **Scale** | minimum support $x_m > 0$ |
| **N** | sample size |

## Usage

Set **Shape**, **Scale**, and **N**, then run the graph. **Samples** is a `DataSeries<Float64>` with values $\ge x_m$. Use for wealth, city-size, and other heavy-tail phenomena and extreme-value modeling.
