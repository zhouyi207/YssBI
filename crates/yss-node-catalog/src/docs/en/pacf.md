# Partial autocorrelation function (PACF)

Connect a time-ordered `series` with at least four non-null finite observations. `lags` ranges from 1–40, default 1; the effective maximum is $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$.

## Formula

Apply the Yule–Walker / Durbin–Levinson recursion to sample ACF:

$$
\phi_{1,1}=\hat\rho_1,\qquad
\phi_{k,k}=
\frac{\hat\rho_k-\sum_{j=1}^{k-1}\phi_{k-1,j}\hat\rho_{k-j}}
{1-\sum_{j=1}^{k-1}\phi_{k-1,j}\hat\rho_j},
$$

$$
\phi_{k,j}=\phi_{k-1,j}-\phi_{k,k}\phi_{k-1,k-j},
\quad j<k,\qquad
\widehat{\mathrm{PACF}}(k)=\phi_{k,k}.
$$

$\hat\rho_k$ is sample autocorrelation at lag $k$ and $\phi_{k,j}$ the recursion coefficient. PACF measures linear association after accounting for intermediate lags.

## Outputs and interpretation

`result` contains the structured result, containing `function=pacf`, `observations=n` and `values`. The array starts at **lag one**: `values[0]` is lag-one PACF. Its usual length is $h$; a constant series returns an empty array.

Lags use row positions without automatic sorting or gap filling. Values help explore time-series structure but include no confidence bands or p-values. Uncomputable recursion fails.

Method details: [Durbin–Levinson recursion](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.levinson_durbin.html).
