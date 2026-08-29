# Gamma

The gamma distribution $\mathrm{Gamma}(\alpha, \beta)$ in shape–rate form has density:

$$
f(x)=\frac{\beta^\alpha}{\Gamma(\alpha)}x^{\alpha-1}e^{-\beta x},\quad x > 0
$$

## Usage

Set **Shape**, **Rate**, and **N**, then run the graph. **Samples** is a positive `DataSeries<Float64>`. Use for sums of waiting times, conjugate priors in Bayesian models, and as the generalization of **Erlang**.
