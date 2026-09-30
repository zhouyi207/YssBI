# Principal component analysis (PCA)

Provide 2–16 numeric **Variable** series with observations in aligned rows. Relational series must share a row domain; missing values are rejected. **Retained dimensions** defaults to 2, with $1\le k\le\min(p,n-1)$ for $p$ variables and $n$ observations.

**Standardize variables** defaults to enabled: center and divide by sample standard deviations with denominator $n-1$. Disabling it uses covariance PCA in centered original units. Standardized PCA rejects constant columns; original-unit PCA permits them but requires positive total variance.

For processed data $Z$, covariance $S=Z^TZ/(n-1)$ and $SV=V\Lambda$:

$$T=ZV_k,\qquad r_j=\lambda_j/\sum_\ell\lambda_\ell.$$

$V_k$ contains the first $k$ unit eigenvectors, $T$ the scores and $r_j$ the explained-variance proportions. PCA is descriptive dimensional reduction without hypothesis tests or p-values. Repeated eigenvalues do not identify unique axis directions; sign changes leave variance and reconstruction unchanged.

**Result** includes `means`, `scales`, variable-by-retained-axis `weights`, all eigenvalues, variance/cumulative proportions, retained variance and numerical rank. The weights are score coefficients, rather than correlation loadings multiplied by square roots of eigenvalues. With standardization disabled, all scales are 1.

**Scores** is a pageable, connectable numeric table with `axis1` through `axisK` in original observation order. Connect a column selector for further plotting or analysis. This independent computed table does not establish alignment with original lazy series by equal row count. Observation scores are kept outside summary JSON.
