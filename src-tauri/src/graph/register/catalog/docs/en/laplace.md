# Laplace

The Laplace (double exponential) distribution $\mathrm{Laplace}(\mu, b)$ has density:

$$
f(x)=\frac{1}{2b}\exp\!\left(-\frac{|x-\mu|}{b}\right)
$$

## Pins

| Pin | Description |
|-----|-------------|
| **Location** | location parameter $\mu$ |
| **Scale** | scale parameter $b > 0$ |
| **N** | sample size |

## Usage

Set **Location**, **Scale**, and **N**, then run the graph. **Samples** is a `DataSeries<Float64>`. Use for peaked, heavy-tailed error terms, robust statistics demos, and comparisons with the normal distribution.
