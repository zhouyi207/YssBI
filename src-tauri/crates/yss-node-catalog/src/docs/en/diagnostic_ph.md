# Proportional-hazards score test

Connect aligned **time** (positive follow-up), **event** (1/true for an event, 0/false for right censoring) and one or more numeric **predictors**. Missing rows are rejected. The node fits a static, unstratified Cox model, then tests time interactions using an efficient score.

**Tied events** defaults to efron, with breslow also available; fitting and testing use the same partial likelihood. **Time transform** defaults to rank (average ranks of all follow-up times), with log and identity alternatives. **Maximum iterations** defaults to 500; **Convergence tolerance** defaults to $10^{-7}$ with supported range $10^{-12}$ to $0.01$.

Consider $\beta(t)=\beta+\gamma g(t)$. $H_0:\gamma=0$ means covariate effects do not vary with the selected time function; the alternative has at least one nonzero interaction. Adjusting for nuisance coefficients gives

$$
U_e=U_\gamma-I_{\gamma\beta}I_{\beta\beta}^{-1}U_\beta,\qquad
I_e=I_{\gamma\gamma}-I_{\gamma\beta}I_{\beta\beta}^{-1}I_{\beta\gamma}.
$$

The global statistic is $U_e^\mathsf{T}I_e^{-1}U_e\mathrel{\dot\sim}\chi_p^2$ for $p$ predictors. Per-variable tests use $U_{e,j}^2/I_{e,jj}\mathrel{\dot\sim}\chi_1^2$, fixing other time interactions at zero.

**result** reports observations, events, ties, time transform, fitted likelihood and coefficients, and term/global statistics, degrees of freedom and p-values. At least two distinct event times, a converged fit and identifiable interaction information are required. Small p-values indicate the tested PH departure; nonrejection does not rule out every form of time variation. [Score-test reference](https://stat.ethz.ch/R-manual/R-devel/library/survival/html/cox.zph.html)
