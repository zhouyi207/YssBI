# Poisson

The Poisson distribution $\mathrm{Poisson}(\lambda)$ models the count of rare events in a fixed interval:

$$
P(X=k)=\frac{e^{-\lambda}\lambda^k}{k!},\quad k=0,1,2,\ldots
$$

## Pins

| Pin | Description |
|-----|-------------|
| **Lambda** | rate parameter $\lambda > 0$ (mean and variance both equal $\lambda$) |
| **N** | sample size |

## Usage

Wire **Lambda** and **N**, then run the graph. **Samples** is a `DataSeries<Int64>` of non-negative integers. Use for event counts per unit time or space, queue arrivals, and rare-event simulation.
