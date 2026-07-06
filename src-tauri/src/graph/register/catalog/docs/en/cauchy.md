# Cauchy

The Cauchy distribution $\mathrm{Cauchy}(\mu, \gamma)$ has heavy tails and no finite mean:

$$
f(x)=\frac{1}{\pi\gamma\left[1+\left(\frac{x-\mu}{\gamma}\right)^2\right]}
$$

## Pins

| Pin | Description |
|-----|-------------|
| **Location** | location parameter $\mu$ |
| **Scale** | scale parameter $\gamma > 0$ |
| **N** | sample size |

## Usage

Set **Location**, **Scale**, and **N**, then run the graph. **Samples** is a `DataSeries<Float64>`. Use for heavy-tail phenomena, robustness demos, and extreme-value simulation (note that the mean does not exist).
