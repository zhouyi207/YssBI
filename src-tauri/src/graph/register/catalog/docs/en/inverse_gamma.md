# Inverse Gamma

The inverse gamma distribution $\mathrm{InvGamma}(\alpha, \beta)$ is the reciprocal of a gamma variable and is common as a variance prior:

$$
f(x)=\frac{\beta^\alpha}{\Gamma(\alpha)}x^{-\alpha-1}e^{-\beta/x},\quad x > 0
$$

## Pins

| Pin | Description |
|-----|-------------|
| **Shape** | shape parameter $\alpha > 0$ |
| **Scale** | scale parameter $\beta > 0$ |
| **N** | sample size |

## Usage

Set **Shape**, **Scale**, and **N**, then run the graph. **Samples** is a positive `DataSeries<Float64>`. Use for variance or precision priors in Bayesian models and simulation of positive reciprocals.
