# Fisher–Snedecor F

The F distribution $\mathrm{F}(d_1, d_2)$ is the ratio of two independent chi-squared variables, each divided by its degrees of freedom:

$$
X=\frac{\chi^2(d_1)/d_1}{\chi^2(d_2)/d_2}
$$

## Pins

| Pin | Description |
|-----|-------------|
| **D1** | numerator degrees of freedom $d_1 > 0$ |
| **D2** | denominator degrees of freedom $d_2 > 0$ |
| **N** | sample size |

## Usage

Set **D1**, **D2**, and **N**, then run the graph. **Samples** is a non-negative `DataSeries<Float64>`. Use for variance-ratio tests, F statistics in ANOVA, and overall regression significance simulation.
