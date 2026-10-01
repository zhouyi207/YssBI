# Heckman two-step selection model

Fits a Probit selection equation followed by an outcome OLS equation augmented with the inverse Mills ratio.

## Inputs and parameters

Connect aligned `response`, binary or numeric 0/1 `selected`, optional numeric `predictors`, and at least one numeric `selection_predictors` column.
True/1 means the outcome is observed. `response` may be null on unselected rows; those values are ignored. Selected outcomes and every selection/covariate value must be finite and complete.
Both selection states must occur.

Both equations include intercepts. Supply the complete selection equation, including outcome predictors when appropriate. At least one selection column must add linear information beyond the outcome design (an exclusion restriction). This numerical rank condition does not establish substantive instrument validity.

`max_iterations=500`, `tolerance=0.0000001` control Probit convergence; iterations must be positive and tolerance in [1e-12,0.01]. `bootstrap_replications=0` disables outcome inference; otherwise use an integer at least 2. `seed=42` is nonnegative.

## Calculation and inference

For selection index $a_i=Z_i'\gamma$, the selected sample uses $\lambda_i=\phi(a_i)/\Phi(a_i)$ and

$$
E[Y_i\mid X_i,Z_i,S_i=1]=X_i'\beta+\delta\lambda_i,
\qquad \delta=\rho\sigma.
$$

$\phi,\Phi$ are the standard normal density and CDF. The scale moment is
$\hat\sigma^2=n_s^{-1}\sum_{S_i=1}[\hat u_i^2+\hat\delta^2\lambda_i(\lambda_i+a_i)]$; $\hat\rho=\hat\delta/\hat\sigma$.
Nonpositive scale or $|\hat\rho|\ge1$ fails instead of clipping the estimate.

Selection coefficients have ordinary Probit information-based normal inference. Outcome covariance, when requested, resamples whole independent rows and refits both stages, accounting for estimated Mills ratios. Failed replications abort. It is not the naive second-stage OLS covariance.

Coefficient tests use $H_0:b_j=0$ versus $H_1:b_j\ne0$, $z=\hat b_j/SE_j$, asymptotic standard normal reference, two-sided p-values and 95% normal intervals. Outcome inference is null with zero bootstrap replications. The Mills coefficient test concerns selection correlation under the maintained model.

## Result and scope

`result` contains selection/outcome coefficients, optional outcome covariance, full-sample selection probabilities, selected-only Mills ratios, one-based selected row indices, conditional fitted outcomes/residuals, sigma and rho.
Rows retain original order. The model assumes jointly normal selection/outcome errors, valid exclusions and independent observations; it is neither full-information MLE nor a binary outcome selection model. See the [Heckman reference](https://www.stata.com/manuals/rheckman.pdf).
