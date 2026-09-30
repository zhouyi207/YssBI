# Breusch–Pagan / Koenker heteroskedasticity test

Connect an OLS or WLS `model` to test whether error variance changes with selected variables. The fitted sample is reused; GLS is unsupported.

| Parameter | Default | Meaning                                                               |
| --------- | ------- | --------------------------------------------------------------------- |
| `rhs`     | `false` | Use an intercept and fitted values; enable to use the original design |
| `koenker` | `false` | Original BP; enable the studentized Koenker form                      |

## Original Breusch–Pagan

- **Null $H_0$:** homoskedasticity; all nonconstant auxiliary variance coefficients are zero.
- **Alternative $H_1$:** at least one is nonzero; variance changes with the selected variables.

Regress standardized squared residuals $g_i=u_i^2/\hat\sigma^2$ on the selected auxiliary variables, where $\hat\sigma^2=\sum_i u_i^2/n$:

$$
LM_{\mathrm{BP}}=\tfrac12\mathrm{ESS}_{\mathrm{aux}}
\overset{H_0}{\approx}\chi_q^2.
$$

$\mathrm{ESS}_{\mathrm{aux}}$ is the auxiliary explained sum of squares. Original BP calibration assumes normal errors.

## Studentized Koenker

- **Null $H_0$:** homoskedasticity; all nonconstant auxiliary variance coefficients are zero.
- **Alternative $H_1$:** at least one is nonzero, indicating variance related to the selected variables.

Use the same auxiliary regression's coefficient of determination:

$$
LM_{\mathrm K}=nR_{\mathrm{aux}}^2
\overset{H_0}{\approx}\chi_q^2.
$$

Koenker relaxes normality but does not automatically handle serial correlation or clustering. $n$ is fitted sample size. The default has $q=1$; RHS mode uses the original column count minus one and requires an intercept for the standard interpretation.

WLS retains original weights $w_i$. BP uses a weighted auxiliary regression of $u_i^2/(\sum_iw_i u_i^2/n)-1$ and statistic $\tfrac12\sum_iw_i\hat f_i^2$, where $\hat f_i$ are its fitted values. Koenker uses weighted auxiliary $R^2$.

## Outputs and interpretation

`result` contains the structured result: `test=breusch_pagan` with inner `result` fields `lm_stat`, `df` and `p_value`. Only the selected variant runs. P-values use the corresponding chi-square upper tail; $p<\alpha$ rejects homoskedasticity.

Default mode requires four observations; RHS mode requires at least the design column count plus two. Zero residual variance or singular auxiliary designs prevent calculation.

Method details: [BP / Koenker](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.het_breuschpagan.html).
