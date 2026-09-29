# Jarque–Bera / Omnibus normality tests

Connect a numeric `series`, such as regression residuals, to run both normality tests. There are no parameters. At least eight non-null, finite observations are required.

## Jarque–Bera test

- **Null $H_0$:** the population is normal.
- **Alternative $H_1$:** the population is nonnormal, detected through departures in skewness or kurtosis.

$$
JB=\frac n6\left[S^2+\frac{(K-3)^2}{4}\right],
\qquad JB\overset{H_0}{\approx}\chi^2_2.
$$

$n$ is sample size, $S$ sample skewness and $K$ Pearson kurtosis, whose normal benchmark is 3. Sample central moments use denominator $n$, without an unbiased correction.

## D’Agostino–Pearson Omnibus test

- **Null $H_0$:** the population is normal.
- **Alternative $H_1$:** the population is nonnormal, detected through departures in skewness or kurtosis.

$$
K^2=Z_S^2+Z_K^2,\qquad K^2\overset{H_0}{\approx}\chi^2_2.
$$

$Z_S$ and $Z_K$ are the sample-size-adjusted standardized skewness and kurtosis statistics. $K^2$ names the test statistic, not squared sample kurtosis. The two methods use different standardizations and can produce different p-values.

## Outputs and interpretation

`result` and `report` return identical results:

| Field                                      | Meaning                            |
| ------------------------------------------ | ---------------------------------- |
| `skewness` / `kurtosis`                    | Sample skewness / Pearson kurtosis |
| `jarque_bera_stat` / `jarque_bera_p_value` | JB statistic and p-value           |
| `omnibus_stat` / `omnibus_p_value`         | Omnibus statistic and p-value      |

Both use upper-tail chi-square p-values with two degrees of freedom. Reject the corresponding null when $p<\alpha$; otherwise evidence is insufficient. The node does not combine the p-values.

Use small-sample asymptotic results cautiously, especially the Omnibus kurtosis component when $n\leq20$. Constant series are unsuitable; any returned values do not establish a normality pass.

Method details: [Jarque–Bera](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.jarque_bera.html), [Omnibus](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.normaltest.html).
