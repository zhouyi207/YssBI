"""Reproduce independent NumPy/SciPy/statsmodels references for causal nodes.

Run with Python, numpy, scipy and statsmodels. No Rust implementation is called.
"""
import json
from pathlib import Path

import numpy as np
import scipy
from scipy.optimize import minimize
from scipy.special import log_ndtr
from scipy.stats import chi2, norm
import statsmodels
import statsmodels.api as sm
from statsmodels.sandbox.regression.gmm import LinearIVGMM
from statsmodels.tools.numdiff import approx_hess

rng = np.random.default_rng(104313)
out = {"versions": {"numpy": np.__version__, "scipy": scipy.__version__, "statsmodels": statsmodels.__version__}}
n = 240
x = rng.normal(size=(n, 2))
d = rng.binomial(1, 1 / (1 + np.exp(-0.1 - x @ [0.3, -0.2])))
y = 2 + x @ [0.7, -0.4] + d * (1.5 + 0.6 * x[:, 0]) + rng.normal(size=n)
X = sm.add_constant(x)
e = sm.Logit(d, X).fit(disp=False).predict()
m = np.array([sm.OLS(y[d == j], X[d == j]).fit().predict(X) for j in (0, 1)])
matched = []
for i in range(n):
    distance = np.where(d != d[i], abs(e - e[i]), np.inf)
    matched.append(y[distance == distance.min()].mean())
matched = np.array(matched)
effects = (2 * d - 1) * (y - matched)
weighted = lambda w: w @ y / w.sum()
out["treatment"] = {"response": y.tolist(), "treatment": d.tolist(), "predictors": x.T.tolist(),
    "propensity": e.tolist(), "cases": {
        "matching": [effects.mean(), effects[d == 1].mean()],
        "ipw": [weighted(d / e) - weighted((1 - d) / (1 - e)), y[d == 1].mean() - weighted((1 - d) * e / (1 - e))],
        "ra": [(m[1] - m[0]).mean(), (m[1] - m[0])[d == 1].mean()],
        "aipw": [(m[1] - m[0] + d * (y - m[1]) / e - (1 - d) * (y - m[0]) / (1 - e)).mean(),
            (d * (y - m[0]) - (1 - d) * e * (y - m[0]) / (1 - e)).sum() / d.sum()]}}

z = rng.normal(size=(n, 3))
u = rng.normal(size=n)
endog = z @ [0.7, 0.9, 0.2] + 0.7 * u + rng.normal(size=n)
X = sm.add_constant(np.column_stack([endog, z[:, 2]]))
Z = sm.add_constant(z)
y = X @ [1.2, 2.1, -0.6] + (1 + 0.3 * z[:, 0] ** 2) * u
cases = []
for steps in (1, 2):
    fit = LinearIVGMM(y, X, Z).fit(maxiter=steps, inv_weights=Z.T @ Z / n,
        weights_method="cov", wargs={"centered": False}, has_optimal_weights=False)
    cases.append({"steps": steps, "coefficients": fit.params.tolist(), "covariance": fit.cov_params().tolist(), "j": fit.jval})
out["gmm"] = {"response": y.tolist(), "predictors": X[:, 1:].T.tolist(), "instruments": z.T.tolist(), "cases": cases}

running = np.linspace(-1.8, 1.8, n)
y = 1.2 + 0.4 * running + (running >= 0) * (2 + 0.9 * running) + rng.normal(scale=0.3, size=n)
cases = []
for triangular in (False, True):
    keep = abs(running) < 1
    r = running[keep]
    X = np.column_stack([np.ones(len(r)), r >= 0, r, r * (r >= 0)])
    w = 1 - abs(r) if triangular else np.ones(len(r))
    fit = sm.WLS(y[keep], X, weights=w).fit(cov_type="HC3", use_t=False)
    cases.append({"triangular": triangular, "coefficients": fit.params.tolist(), "covariance": fit.cov_params().tolist()})
out["rdd"] = {"response": y.tolist(), "running": running.tolist(), "cases": cases}

t = out["treatment"]
y = np.array(t["response"])
groups = (x[:, 0] > 0).astype(int)
X = np.column_stack([np.ones(n), x, d, groups, d * groups])
fit = sm.OLS(y, X).fit(cov_type="HC3", use_t=False)
out["heterogeneity"] = {"groups": groups.tolist(), "coefficients": fit.params.tolist(), "covariance": fit.cov_params().tolist(),
    "statistic": fit.params[-1] ** 2 / fit.cov_params()[-1, -1]}

n = 400
x = rng.normal(size=n)
z = rng.normal(size=n)
v = rng.normal(size=n)
u = 0.3 * v + np.sqrt(1 - 0.3 ** 2) * rng.normal(size=n)
selected = (0.5 + 0.3 * x + 0.9 * z + v > 0).astype(int)
y = 1.1 + 1.6 * x + u
Z = np.column_stack([np.ones(n), x, z])
probit = sm.Probit(selected, Z).fit(disp=False)
eta = Z @ probit.params
keep = selected == 1
mills = norm.pdf(eta[keep]) / norm.cdf(eta[keep])
X = np.column_stack([np.ones(keep.sum()), x[keep], mills])
second = sm.OLS(y[keep], X).fit()
sigma = np.sqrt(np.mean(second.resid ** 2) + second.params[-1] ** 2 * np.mean(mills * (mills + eta[keep])))
out["heckman"] = {"response": [float(v) if s else None for v, s in zip(y, selected)], "selected": selected.tolist(),
    "predictors": [x.tolist()], "selection_predictors": [x.tolist(), z.tolist()], "coefficients": second.params.tolist(),
    "selection_coefficients": probit.params.tolist(), "sigma": sigma, "rho": second.params[-1] / sigma}

n = 360
x = rng.normal(size=(n, 2))
X = sm.add_constant(x)
y = X @ [2.0, 0.6, -0.3] + rng.normal(scale=0.35, size=n) - abs(rng.normal(scale=1.2, size=n))
def frontier_loss(b):
    su, sv = np.exp(b[-2:])
    sigma = np.hypot(su, sv)
    e = (y - X @ b[:-2]) / sigma
    return np.mean(np.log(sigma) + 0.5 * e ** 2 + 0.5 * np.log(2 * np.pi) - np.log(2) - log_ndtr(-e * su / sv))
fit = minimize(frontier_loss, [2, 0.6, -0.3, np.log(1.2), np.log(0.35)], method="BFGS", options={"gtol": 1e-9})
assert np.linalg.norm(fit.jac, np.inf) < 1e-6, fit.message
cov = np.linalg.inv(approx_hess(fit.x, frontier_loss)) / n
su, sv = np.exp(fit.x[-2:])
residuals = y - X @ fit.x[:-2]
mu = -residuals * su ** 2 / (su ** 2 + sv ** 2)
sd = su * sv / np.hypot(su, sv)
efficiency = np.exp(-mu + sd ** 2 / 2 + log_ndtr(mu / sd - sd) - log_ndtr(mu / sd))
out["frontier"] = {"response": y.tolist(), "predictors": x.T.tolist(), "coefficients": fit.x[:-2].tolist(),
    "sigma_u": su, "sigma_v": sv, "log_likelihood": -fit.fun * n, "covariance": cov[:-2, :-2].tolist(), "efficiency": efficiency.tolist()}

n = 100
x = rng.normal(size=(n, 3))
errors = rng.multivariate_normal([0, 0], [[1, 0.7], [0.7, 1.5]], n)
designs = [sm.add_constant(x[:, [0, 1]]), sm.add_constant(x[:, [0, 2]])]
responses = [designs[0] @ [1, 0.5, 1.2] + errors[:, 0], designs[1] @ [-0.5, 1.1, -0.7] + errors[:, 1]]
residuals = np.array([sm.OLS(y, X).fit().resid for y, X in zip(responses, designs)])
sigma = residuals @ residuals.T / n
omega_inv = np.kron(np.linalg.inv(sigma), np.eye(n))
stacked_x = scipy.linalg.block_diag(*designs)
cov = np.linalg.inv(stacked_x.T @ omega_inv @ stacked_x)
beta = cov @ stacked_x.T @ omega_inv @ np.concatenate(responses)
out["sur"] = {"responses": [y.tolist() for y in responses], "predictors": x.T.tolist(),
    "coefficients": beta.tolist(), "covariance": cov.tolist(), "error_covariance": sigma.tolist()}

n, pre = 32, 20
donors = rng.normal(size=(n, 3)).cumsum(axis=0)
y = donors @ [0.25, 0.75, 0] + rng.normal(scale=0.03, size=n)
y[pre:] += 1.7
fit = minimize(lambda w: np.mean((y[:pre] - donors[:pre] @ w) ** 2), np.ones(3) / 3, method="SLSQP",
    bounds=[(0, 1)] * 3, constraints={"type": "eq", "fun": lambda w: w.sum() - 1}, options={"ftol": 1e-13, "maxiter": 5000})
assert fit.success, fit.message
out["synthetic"] = {"response": y.tolist(), "donors": donors.T.tolist(), "pre_periods": pre,
    "weights": fit.x.tolist(), "post_effect": np.mean(y[pre:] - donors[pre:] @ fit.x)}

Path(__file__).with_suffix(".json").write_text(json.dumps(out, ensure_ascii=False, indent=2, allow_nan=False) + "\n", encoding="utf-8")
