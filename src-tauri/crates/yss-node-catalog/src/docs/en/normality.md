# Jarque–Bera / Omnibus normality tests

Computes Jarque–Bera (JB) and D’Agostino–Pearson Omnibus ($K^2$) tests on the same series. Both detect departures from normality through skewness and kurtosis, using different standardizations, so their statistics and p-values can differ.

## Input and applicability

Connect a numeric `series`, such as linear regression `residuals`. There are no parameters; both tests always run. At least eight non-null, finite observations are required. Missing values are not automatically dropped. The test concerns the connected series: connecting an original variable tests that variable, while connecting residuals diagnoses their distribution.

The reference distributions assume independent, identically distributed observations from a nondegenerate normal population. For fitted residuals, interpret the results as model diagnostics; serial correlation, heteroskedasticity, clustering and outliers can affect inference. The execution minimum does not ensure an accurate approximation: JB is asymptotic, and the Omnibus kurtosis transformation also needs caution in small samples, especially $n\leq20$. See the [SciPy JB notes](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.jarque_bera.html) and [kurtosis-test notes](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.kurtosistest.html).

## Notation and sample moments

For observations $x_1,\ldots,x_n$, define

$$
\bar x=\frac1n\sum_{i=1}^n x_i,\qquad
m_r=\frac1n\sum_{i=1}^n(x_i-\bar x)^r,
\qquad S=\frac{m_3}{m_2^{3/2}},\qquad K=\frac{m_4}{m_2^2}.
$$

$S$ is sample skewness and $K$ is Pearson kurtosis; their normal-population counterparts are 0 and 3. Central moments use denominator $n$, without an $n-1$ or unbiased-skewness correction. Excess kurtosis is $K-3$.

## Jarque–Bera test

**Null hypothesis $H_0$:** the population is normal with unknown mean and positive variance; its skewness is 0 and Pearson kurtosis is 3.

**Alternative hypothesis $H_1$:** the population is nonnormal. This test detects departures through skewness or kurtosis. Matching both normal moments alone does not establish normality.

The statistic and p-value are

$$
JB=\frac n6\left[S^2+\frac{(K-3)^2}{4}\right],
\qquad JB\overset{H_0}{\approx}\chi^2_2,
\qquad p_{\mathrm{JB}}=1-F_{\chi^2_2}(JB).
$$

$F_{\chi^2_2}$ is the chi-square CDF with two degrees of freedom. Larger values provide more evidence against $H_0$. This node uses the asymptotic statistic above, without a small-sample correction.

## D’Agostino–Pearson Omnibus test

**Null hypothesis $H_0$:** the population is nondegenerate normal.

**Alternative hypothesis $H_1$:** the population is nonnormal, with skewness or kurtosis inconsistent with normal sampling.

Transform skewness and kurtosis separately into approximately standard-normal statistics $Z_S$ and $Z_K$, then combine them:

$$
K^2=Z_S^2+Z_K^2,\qquad
K^2\overset{H_0}{\approx}\chi^2_2,\qquad
p_{\mathrm{Omnibus}}=1-F_{\chi^2_2}(K^2).
$$

$K^2$ is the conventional name of the Omnibus statistic, not the square of sample kurtosis $K$. The transformations depend on sample size; substituting $S$ and $K-3$ directly is incorrect. See [SciPy normaltest](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.normaltest.html).

### Skewness transformation

This component assesses zero population skewness against a two-sided nonzero alternative. Compute

$$
Y=S\sqrt{\frac{(n+1)(n+3)}{6(n-2)}},\qquad
B=\frac{3(n^2+27n-70)(n+1)(n+3)}
{(n-2)(n+5)(n+7)(n+9)},
$$

$$
W^2=-1+\sqrt{2(B-1)},\quad
\delta=\left(\tfrac12\ln W^2\right)^{-1/2},\quad
a=\sqrt{\frac{2}{W^2-1}},\quad
Z_S=\delta\ln\left[\frac Ya+\sqrt{1+\left(\frac Ya\right)^2}\right].
$$

The current SciPy-style numerical convention replaces an exactly zero $Y$ with 1 in the final expression. Thus a perfectly symmetric sample need not have a zero Omnibus skewness component; preserve this convention when reproducing results. See [SciPy skewtest](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.skewtest.html).

### Kurtosis transformation

This component assesses population Pearson kurtosis 3 against a two-sided alternative. Start with the finite-sample normal mean and variance of sample kurtosis:

$$
E=\frac{3(n-1)}{n+1},\qquad
V=\frac{24n(n-2)(n-3)}{(n+1)^2(n+3)(n+5)},\qquad
X=\frac{K-E}{\sqrt V}.
$$

The Anscombe–Glynn transformation is

$$
b=\frac{6(n^2-5n+2)}{(n+7)(n+9)}
\sqrt{\frac{6(n+3)(n+5)}{n(n-2)(n-3)}},
\qquad
A=6+\frac8b\left(\frac2b+\sqrt{1+\frac4{b^2}}\right),
$$

$$
D=1+X\sqrt{\frac2{A-4}},\qquad
Z_K=
\frac{1-\frac2{9A}
-\operatorname{sgn}(D)\left(\frac{1-2/A}{|D|}\right)^{1/3}}
{\sqrt{2/(9A)}}.
$$

The signed cube root preserves the sign of $D$. For the extreme case $|D|<10^{-300}$, the current calculation returns $Z_K=X$. These two transformed components are intermediate quantities; the node does not output separate component tests.

## Outputs

`result` and `report` contain the same computation.

| Field                                      | Meaning                                                |
| ------------------------------------------ | ------------------------------------------------------ |
| `skewness`                                 | $S$: positive indicates right skew, negative left skew |
| `kurtosis`                                 | Pearson kurtosis $K$, not excess kurtosis              |
| `jarque_bera_stat` / `jarque_bera_p_value` | JB statistic and upper-tail p-value                    |
| `omnibus_stat` / `omnibus_p_value`         | Omnibus $K^2$ statistic and upper-tail p-value         |

## Interpretation and example

Choose a significance level $\alpha$, such as 0.05, before inspecting results. For each test, reject normality when $p<\alpha$; otherwise there is insufficient evidence to reject it. Non-rejection does not prove normality, and a p-value is not the probability that the data are normal.

For example, $n=100$, $S=0.6$ and $K=4$ give $JB\approx10.167$ and $p_{\mathrm{JB}}\approx0.0062$, rejecting at 5%. Omnibus still requires its own sample-size transformations and p-value. The two tests share the sample and are not independent confirmations. This node neither combines their p-values nor adjusts for multiple testing.

Use Q–Q plots, histograms, outlier checks and model context alongside these tests. Small samples may have low power; large samples may detect small, practically unimportant departures.

For a constant series, $m_2=0$ makes skewness and kurtosis mathematically undefined. The current calculation substitutes $S=0$, $K=3$ and continues; its output must not be interpreted as passing normality. Insufficient observations, nulls, nonfinite inputs or nonfinite results fail.
