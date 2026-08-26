# Weibull

The Weibull distribution $\mathrm{Weibull}(k, \lambda)$ is widely used in reliability analysis:

$$
f(x)=\frac{k}{\lambda}\left(\frac{x}{\lambda}\right)^{k-1}\exp\!\left[-\left(\frac{x}{\lambda}\right)^k\right],\quad x \ge 0
$$

## Usage

Set **Shape**, **Scale**, and **N**, then run the graph. **Samples** is a non-negative `DataSeries<Float64>`. Use for lifetime and failure-time modeling; when $k=1$ it reduces to the exponential distribution.
