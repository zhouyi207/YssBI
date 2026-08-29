# Fisher–Snedecor F

The F distribution $\mathrm{F}(d_1, d_2)$ is the ratio of two independent chi-squared variables, each divided by its degrees of freedom:

$$
X=\frac{\chi^2(d_1)/d_1}{\chi^2(d_2)/d_2}
$$

## Usage

Set **D1**, **D2**, and **N**, then run the graph. **Samples** is a non-negative `DataSeries<Float64>`. Use for variance-ratio tests, F statistics in ANOVA, and overall regression significance simulation.
