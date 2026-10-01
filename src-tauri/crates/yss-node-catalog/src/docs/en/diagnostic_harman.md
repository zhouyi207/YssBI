# Harman single-factor diagnostic

Connect all questionnaire items of interest for the same respondents to **variables**. At least two numeric items and two observations are required. Columns must align and vary; missing or nonfinite values fail.

This node specifically uses **unrotated principal components of the correlation matrix**. Items are centered and scaled by sample standard deviations before decomposition. There are no parameters and no confirmatory factor analysis.

For ordered correlation eigenvalues $\lambda_1\geq\cdots\geq\lambda_p$, the first component explains $\lambda_1/\sum_j\lambda_j$ of total variance. Its loadings are $\sqrt{\lambda_1}v_1$, where $v_1$ is the unit eigenvector.

**result** contains extraction, observations, variables, eigenvalues, explained_variance_ratio, first_component_ratio, first_component_loadings and eigenvalues_above_one. Loadings follow input-item order; variance ratios are between zero and one.

A large first-component proportion indicates concentrated shared variation. This diagnostic alone cannot establish or exclude common-method bias. It supplies neither a p-value nor an automatic pass/fail threshold.
