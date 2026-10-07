# Correlogram (ACF & PACF)

Values requires at least 4 finite, nonconstant Numeric observations and rejects missing values. Maximum lag defaults to 20 and ranges from 1 to 40; the effective maximum is $\min(L,\lfloor n/2\rfloor-1,40)$.

Displays ACF and Durbin–Levinson PACF starting at lag 1, with a 95% white-noise reference half-width $1.96/\sqrt n$. ACF includes cumulative Ljung–Box $Q_k=n(n+2)\sum_{j=1}^k r_j^2/(n-j)$, testing zero autocorrelation through lag $k$ using $\chi^2_k$ P values without fitted-parameter adjustment. PACF does not carry a Q test. Open Result in a result panel or Plot window for both panels.
