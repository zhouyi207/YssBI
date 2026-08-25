# Student's t

Student's t distribution with degrees of freedom $\nu$ has heavier tails than the normal. This node uses location 0 and scale 1:

$$
f(x)=\frac{\Gamma\!\left(\frac{\nu+1}{2}\right)}{\sqrt{\nu\pi}\,\Gamma\!\left(\frac{\nu}{2}\right)}\left(1+\frac{x^2}{\nu}\right)^{-\frac{\nu+1}{2}}
$$

## Pins

| Pin | Description |
|-----|-------------|
| **DF** | degrees of freedom $\nu > 0$ |
| **N** | sample size |

## Usage

Wire **DF** and **N**, then run the graph. **Samples** is a `DataSeries<Float64>`. Use for heavy-tailed random errors, small-sample $t$-statistic simulation, and tail comparisons with the normal distribution.
