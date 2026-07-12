# Uniform

The continuous uniform distribution $\mathrm{Uniform}(a,b)$ has constant density on $[a,b)$:

$$
f(x)=\frac{1}{b-a},\quad a \le x < b
$$

## Pins

| Pin | Description |
|-----|-------------|
| **Low** | lower bound $a$ |
| **High** | upper bound $b$, with $a < b$ |
| **N** | sample size |

## Usage

Set **Low**, **High**, and **N**, then run the graph. **Samples** is a `DataSeries<Float64>`. Use for non-informative priors, randomization baselines, and uniform random number generation in Monte Carlo workflows.
