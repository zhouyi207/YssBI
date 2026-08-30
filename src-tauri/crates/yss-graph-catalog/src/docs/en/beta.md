# Beta

The beta distribution $\mathrm{Beta}(\alpha, \beta)$ is supported on $(0,1)$ and models proportions and probabilities:

$$
f(x)=\frac{x^{\alpha-1}(1-x)^{\beta-1}}{B(\alpha,\beta)},\quad 0 < x < 1
$$

## Usage

Set **Alpha**, **Beta**, and **N**, then run the graph. **Samples** is a `DataSeries<Float64>` with values in $(0,1)$. Use for probability priors, proportion uncertainty, and random weights on the unit interval.
