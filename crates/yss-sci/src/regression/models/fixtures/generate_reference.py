"""Reproduce independent regression references with SciPy/statsmodels, without Rust code."""
import json
from pathlib import Path

import numpy as np
import scipy
from scipy.optimize import minimize, least_squares
from scipy.special import log_ndtr
import statsmodels
import statsmodels.api as sm
from statsmodels.discrete.count_model import ZeroInflatedPoisson, ZeroInflatedNegativeBinomialP
from statsmodels.discrete.conditional_models import ConditionalLogit
from statsmodels.miscmodels.ordinal_model import OrderedModel
from statsmodels.othermod.betareg import BetaModel
from statsmodels.tools.numdiff import approx_hess

rng = np.random.default_rng(20260930)
n = 160
x = rng.normal(size=(n, 2)) * [1.7, 0.8] + [2.0, -0.5]
design = sm.add_constant(x)
y = design @ [1.2, 0.7, -0.8] + rng.normal(scale=0.6, size=n)
data = {"source": {"scipy": scipy.__version__, "statsmodels": statsmodels.__version__, "seed": 20260930}, "linear": {"x": x.T.tolist(), "y": y.tolist()}, "cases": {}}

def record(name, response, fit, predictors=x):
    data["cases"][name] = {"x": predictors.T.tolist(), "y": np.asarray(response).tolist(), "coefficients": np.asarray(fit.params).tolist(), "standard_errors": np.asarray(fit.bse).tolist(), "fitted": np.asarray(fit.predict()).tolist()}
    try:
        ll = float(fit.llf)
        if np.isfinite(ll):
            data["cases"][name]["log_likelihood"] = ll
    except (AttributeError, NotImplementedError):
        pass

outliers = y.copy()
outliers[::13] += 9.0
for name, loss in [("huber", sm.robust.norms.HuberT(1.345)), ("tukey", sm.robust.norms.TukeyBiweight(4.685))]:
    fit = sm.RLM(outliers, design, M=loss).fit(maxiter=1000, tol=1e-10, cov="H1", conv="coefs")
    record(name, outliers, fit)
record("quantile", outliers, sm.QuantReg(outliers, design).fit(q=0.3, max_iter=10000, p_tol=1e-10))

z = (x - x.mean(axis=0)) / x.std(axis=0)
centered_y = y - y.mean()
lam = 0.17
ridge = np.linalg.solve(z.T @ z / n + lam * np.eye(2), z.T @ centered_y / n)
ridge_raw = ridge / x.std(axis=0)
data["cases"]["ridge"] = {"coefficients": [float(y.mean() - x.mean(axis=0) @ ridge_raw), *ridge_raw.tolist()], "lambda": lam}
lasso = sm.OLS(centered_y, z).fit_regularized(alpha=lam, L1_wt=1, maxiter=10000, cnvrg_tol=1e-12)
lasso_raw = lasso.params / x.std(axis=0)
data["cases"]["lasso"] = {"coefficients": [float(y.mean() - x.mean(axis=0) @ lasso_raw), *lasso_raw.tolist()], "lambda": lam}
# One-component PLS1 has a closed-form covariance-direction solution.
w = z.T @ centered_y
w /= np.linalg.norm(w)
t = z @ w
q = t @ centered_y / (t @ t)
pls_raw = w * q / x.std(axis=0)
data["cases"]["pls"] = {"coefficients": [float(y.mean() - x.mean(axis=0) @ pls_raw), *pls_raw.tolist()]}

eta = design @ [-0.4, 0.3, -0.5]
binary = rng.binomial(1, 1 / (1 + np.exp(-eta)))
for link in [sm.families.links.Logit(), sm.families.links.Probit(), sm.families.links.CLogLog()]:
    record(type(link).__name__.lower(), binary, sm.GLM(binary, design, family=sm.families.Binomial(link=link)).fit(tol=1e-10, maxiter=1000))
fraction = rng.beta(2 + np.exp(eta), 3, size=n)
record("fractional", fraction, sm.GLM(fraction, design, family=sm.families.Binomial()).fit(tol=1e-10, cov_type="HC0"))
counts = rng.poisson(np.exp(design @ [-0.1, 0.2, -0.3]))
record("poisson", counts, sm.GLM(counts, design, family=sm.families.Poisson()).fit(tol=1e-10))
record("gaussian", y, sm.GLM(y, design, family=sm.families.Gaussian()).fit(tol=1e-10))
positive = rng.gamma(3, np.exp(design @ [0.1, 0.1, -0.2]) / 3)
for name, family in [("gamma", sm.families.Gamma(link=sm.families.links.Log())), ("inverse_gaussian", sm.families.InverseGaussian(link=sm.families.links.Log())), ("gaussian_log", sm.families.Gaussian(link=sm.families.links.Log()))]:
    record(name, positive, sm.GLM(positive, design, family=family).fit(tol=1e-10, maxiter=1000))
nb = rng.negative_binomial(1.1, 1.1 / (1.1 + np.exp(design @ [0.1, 0.25, -0.4])))
record("negative_binomial", nb, sm.NegativeBinomial(nb, design).fit(disp=False, method="bfgs", maxiter=2000, gtol=1e-8))
inflation = rng.random(n) < 0.3
zip_y = np.where(inflation, 0, rng.poisson(np.exp(design @ [0.7, 0.2, -0.3])))
zinb_y = np.where(inflation, 0, rng.negative_binomial(1.3, 1.3 / (1.3 + np.exp(design @ [0.7, 0.2, -0.3]))))
for name, response, cls in [("zip", zip_y, ZeroInflatedPoisson), ("zinb", zinb_y, ZeroInflatedNegativeBinomialP)]:
    fit = cls(response, design, exog_infl=np.ones((n, 1))).fit(disp=False, method="bfgs", maxiter=2000, gtol=1e-8)
    record(name, response, fit)
    # Host order: count coefficients, inflation coefficients, then dispersion.
    order = [1, 2, 3, 0] + ([4] if name == "zinb" else [])
    data["cases"][name]["coefficients"] = fit.params[order].tolist()
    observed_covariance = np.linalg.inv(-approx_hess(fit.params, fit.model.loglike))
    data["cases"][name]["standard_errors"] = np.sqrt(np.diag(observed_covariance))[order].tolist()
beta_y = rng.beta(1 / (1 + np.exp(-eta)) * 7, (1 - 1 / (1 + np.exp(-eta))) * 7)
beta_fit = BetaModel(beta_y, design).fit(disp=False, maxiter=1000, gtol=1e-8)
record("beta", beta_y, beta_fit)
data["cases"]["beta"]["coefficients"][-1] = float(np.exp(beta_fit.params[-1]))
data["cases"]["beta"]["standard_errors"][-1] *= float(np.exp(beta_fit.params[-1]))

latent = design @ [-0.7, 0.35, -0.4] + rng.normal(scale=0.7, size=n)
for name, upper in [("tobit", None), ("tobit_both", 1.4)]:
    observed = np.maximum(latent, 0) if upper is None else np.clip(latent, 0, upper)
    def negative_ll(b):
        mu = design @ b[:-1]
        sigma = np.exp(b[-1])
        ll = -0.5 * ((observed - mu) / sigma) ** 2 - b[-1] - 0.5 * np.log(2 * np.pi)
        ll[observed == 0] = log_ndtr(-mu[observed == 0] / sigma)
        if upper is not None:
            ll[observed == upper] = log_ndtr((mu[observed == upper] - upper) / sigma)
        return -ll.sum()
    fit = minimize(negative_ll, [-0.5, 0.3, -0.3, -0.3], method="BFGS", options={"gtol": 1e-7})
    coefficients = fit.x.copy(); coefficients[-1] = np.exp(coefficients[-1])
    data["cases"][name] = {"x": x.T.tolist(), "y": observed.tolist(), "coefficients": coefficients.tolist(), "log_likelihood": -float(fit.fun)}

mn_prob = np.column_stack([np.ones(n), np.exp(design @ [-0.3, 0.3, -0.4]), np.exp(design @ [0.2, -0.2, 0.3])])
mn_prob /= mn_prob.sum(axis=1)[:, None]
mn = np.array([rng.choice(3, p=row) for row in mn_prob])
mn_fit = sm.MNLogit(mn, design).fit(disp=False, tol=1e-10, maxiter=1000)
record("multinomial", mn, mn_fit)
data["cases"]["multinomial"]["coefficients"] = mn_fit.params.T.ravel().tolist()
data["cases"]["multinomial"]["standard_errors"] = mn_fit.bse.T.ravel().tolist()
ordinal = np.digitize(x @ [0.4, -0.5] + rng.logistic(size=n), [-0.5, 1.0])
ordinal_model = OrderedModel(ordinal, x, distr="logit")
ordinal_fit = ordinal_model.fit(disp=False, method="bfgs", maxiter=2000, gtol=1e-8)
record("ordinal", ordinal, ordinal_fit)
data["cases"]["ordinal"]["coefficients"] = [*ordinal_fit.params[:2], *ordinal_model.transform_threshold_params(ordinal_fit.params)[1:-1]]
jac = np.eye(4); jac[3, 2] = 1; jac[3, 3] = np.exp(ordinal_fit.params[3])
data["cases"]["ordinal"]["standard_errors"] = np.sqrt(np.diag(jac @ ordinal_fit.cov_params() @ jac.T)).tolist()
groups = np.repeat(np.arange(40), 4)
conditional_y = rng.binomial(1, 1 / (1 + np.exp(-(x @ [0.35, -0.4] + rng.normal(size=40)[groups]))))
conditional_fit = ConditionalLogit(conditional_y, x, groups=groups).fit(disp=False, maxiter=1000, gtol=1e-8)
data["cases"]["conditional"] = {"x": x.T.tolist(), "y": conditional_y.tolist(), "groups": groups.tolist(), "coefficients": conditional_fit.params.tolist(), "standard_errors": conditional_fit.bse.tolist(), "log_likelihood": float(conditional_fit.llf)}

nonlinear_x = np.linspace(0.2, 4, 30)
nonlinear_y = 2.4 * np.exp(0.3 * nonlinear_x) + rng.normal(scale=0.08, size=30)
fit = least_squares(lambda b: b[0] * np.exp(b[1] * nonlinear_x) - nonlinear_y, [2.0, 0.2], xtol=1e-13, ftol=1e-13, gtol=1e-13)
data["cases"]["nonlinear"] = {"x": [nonlinear_x.tolist()], "y": nonlinear_y.tolist(), "coefficients": fit.x.tolist()}
Path(__file__).with_name("reference.json").write_text(json.dumps(data, indent=2, allow_nan=False), encoding="utf-8")
print("Generated", len(data["cases"]), "reference cases")
