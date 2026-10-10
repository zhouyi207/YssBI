# Normal–half-normal stochastic frontier

Maximum-likelihood estimation of a cross-sectional production or cost frontier.

## Inputs and parameters

Connect complete, aligned numeric `Y` and optional `X₁, X₂, …`. Supply already transformed data if a log-production or log-cost model is intended; the node does not take logarithms automatically.

`constant=true`, `frontier_type=production`, `max_iterations=500` and `tolerance=0.0000001` are defaults. Choose `cost` to reverse the inefficiency sign. Iterations must be positive and tolerance in [1e-12,0.01].
The design must be full rank and the sample count must exceed the regression coefficient count plus two scale parameters. Unidentified/near-zero variance components, nonpositive information matrices and nonconvergence fail explicitly.

## Model and inference

$$
Y_i=X_i'\beta+v_i-su_i,\quad
v_i\sim N(0,\sigma_v^2),\quad
u_i\sim |N(0,\sigma_u^2)|,
$$

with independent components and independent observations. $s=1$ denotes production; $s=-1$ denotes cost. Let $\sigma^2=\sigma_u^2+\sigma_v^2$, $\lambda=\sigma_u/\sigma_v$, and $\epsilon_i=Y_i-X_i'\beta$. The log-likelihood contribution is

$$
\ell_i=\log(2/\sigma)+\log\phi(\epsilon_i/\sigma)
+\log\Phi(-s\lambda\epsilon_i/\sigma).
$$

Both positive scales are estimated jointly. The full observed information, including regression/scale cross-blocks, yields coefficient covariance. Coefficient tests use $H_0:\beta_j=0$ versus $H_1:\beta_j\ne0$, normal $z=\hat\beta_j/SE_j$, two-sided p-values and 95% intervals. No regular zero-variance Wald test is reported.

## Result

`frontier` is $X\hat\beta$, not the conditional outcome mean. `residuals` is observed outcome minus frontier. `conditional_inefficiency` is $E[u_i\mid\epsilon_i]$ and `efficiency` is $E[\exp(-u_i)\mid\epsilon_i]$, also interpreted as a cost-efficiency ratio for cost models.
These are different from exponentiating expected inefficiency. The result also includes coefficients, covariance, `sigma_u`, `sigma_v`, log likelihood and iterations.
All observation arrays preserve input order. Only homoskedastic half-normal inefficiency is implemented, without panel effects or exponential/truncated-normal variants. See the [frontier model reference](https://www.stata.com/manuals/rfrontier.pdf).
