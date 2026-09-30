# IV Hausman endogeneity test

Connect an IV 2SLS `model` with `nonrobust` covariance to compare IV and OLS coefficients on the same sample. There are no parameters. This tests IV endogeneity, not panel FE/RE differences.

## Hypotheses and formula

- **Null $H_0$:** tested regressors are exogenous; OLS and IV are consistent and OLS is efficient.
- **Alternative $H_1$:** at least one tested regressor is endogenous; OLS is inconsistent while IV remains consistent with valid instruments.

$$
\Delta=\hat\beta_{\mathrm{IV}}-\hat\beta_{\mathrm{OLS}},\qquad
A=V_{\mathrm{IV}}-V_{\mathrm{OLS}},
$$

$$
H=\Delta'A^+\Delta,\qquad
q=\operatorname{rank}(A),\qquad
H\overset{H_0}{\approx}\chi_q^2.
$$

$V_{\mathrm{IV}}$ and $V_{\mathrm{OLS}}$ use a common OLS disturbance variance, the `sigmamore` convention. $A^+$ is a generalized inverse. The comparison includes any fitted intercept, with degrees of freedom given by the covariance difference's effective rank.

## Outputs and interpretation

`result` contains the structured result, containing `hausman` with `stat`, `df` and `p_value`. P-values use the chi-square upper tail; $p<\alpha$ rejects exogeneity.

Inference requires valid, relevant instruments and identification. Non-rejection does not establish exogeneity. Robust/clustered covariance or an unavailable valid OLS comparison fails.

Method details: [Hausman test](https://www.stata.com/manuals/rhausman.pdf).
