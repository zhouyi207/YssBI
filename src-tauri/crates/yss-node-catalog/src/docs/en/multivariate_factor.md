# Exploratory factor analysis

Provide at least three aligned numeric **Variable** series sharing a relational row domain. Missing values and constant columns are rejected. Iterative principal-axis factoring uses a correlation matrix and squared-multiple-correlation initial communalities. PCA extraction and maximum likelihood are not used. The observation count must exceed the variable count $p$, with positive-definite correlation.

**Retained dimensions** defaults to 1 factor. The factor count $k$ must satisfy $k<p$ and identification condition $[(p-k)^2-(p+k)]/2\ge0$. **Maximum iterations** defaults to 500 (positive integer); **convergence tolerance** defaults to $10^{-6}$ in $(0,0.1]$. Extraction and rotation use the same limit. Nonconvergence and Heywood solutions with communality at least 1 fail.

Replacing correlation diagonal entries with communalities, the leading $k$ nonnegative eigenvalues/vectors produce loadings $L$ and updates $h_i^2=\sum_jL_{ij}^2$. **Factor rotation** defaults to orthogonal `varimax`, with `none` available and no Kaiser normalization:

$$Q(L)=\sum_j\left[\sum_iL_{ij}^4-\frac{1}{p}\left(\sum_iL_{ij}^2\right)^2\right].$$

**Result** reports loadings, communalities, uniquenesses $1-h_i^2$, factor variance proportions, iterations and KMO. KMO compares squared correlations with squared partial correlations; a zero denominator gives null. Bartlett's sphericity test has $H_0:R=I$, versus $H_1:R\ne I$:

$$\chi^2=-\left[n-1-\frac{2p+5}{6}\right]\log|R|\ \approx\chi^2_{p(p-1)/2}.$$

$n$ is the observation count. A small Bartlett p-value supports correlation without establishing factor-model fit. No overall factor-model fit test is reported.

**Scores** uses regression scores $\widehat F=ZR^{-1}L$, with $Z$ standardized using sample standard deviations. This independent computed table contains `axis1` through `axisK` in input row order and can feed a column selector. Rotation or sign changes affect axis representation without changing common covariance.

Reference: [statsmodels Factor](https://www.statsmodels.org/stable/generated/statsmodels.multivariate.factor.Factor.html).
