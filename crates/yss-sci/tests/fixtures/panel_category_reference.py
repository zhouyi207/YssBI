"""Regenerate reference.json with NumPy 2.4.2, SciPy 1.17.1, statsmodels 0.14.6.

Fisher combines statsmodels ADF/Engle-Granger p-values using scipy.stats.combine_pvalues.
Difference GMM is independently optimized by statsmodels GMM over entity moments;
its general nonoptimal-weight sandwich supplies the entity-robust covariance.
"""
import json
from pathlib import Path

import numpy as np
import scipy
from scipy.stats import combine_pvalues, norm
import statsmodels
from statsmodels.sandbox.regression.gmm import GMM
from statsmodels.tsa.stattools import adfuller, coint

rng = np.random.default_rng(2601001)
entity, time, response, predictors = [], [], [], [[] for _ in range(5)]
for unit, periods in enumerate([45, 49, 53]):
    x = rng.normal(size=(periods, 5)).cumsum(axis=0)
    u = rng.normal(size=periods)
    for t in range(1, periods):
        u[t] += 0.45 * u[t - 1]
    y = 1.5 + 1.2 * x[:, 0] - 0.7 * x[:, 1] + u
    response.extend(y.tolist())
    entity.extend([float(unit)] * periods)
    time.extend(map(float, range(periods)))
    for j in range(5):
        predictors[j].extend(x[:, j].tolist())

tests = []
for method, regression, width, lags in [
    ("unit_root", "none", 0, 0),
    ("unit_root", "constant", 0, 1),
    ("unit_root", "trend", 0, 1),
    ("cointegration", "constant", 2, 1),
    ("cointegration", "trend", 5, 1),
]:
    rows = []
    for unit in range(3):
        mask = np.asarray(entity) == unit
        y = np.asarray(response)[mask]
        if method == "unit_root":
            statistic, pvalue, _, nobs, *_ = adfuller(
                y, maxlag=lags, autolag=None,
                regression={"none": "n", "constant": "c", "trend": "ct"}[regression],
            )
        else:
            x = np.asarray(predictors[:width]).T[mask]
            statistic, pvalue, _ = coint(
                y, x, trend={"constant": "c", "trend": "ct"}[regression],
                maxlag=lags, autolag=None,
            )
            nobs = len(y) - 1 - lags
        rows.append(dict(statistic=float(statistic), p_value=float(pvalue), observations=nobs))
    fisher = combine_pvalues([row["p_value"] for row in rows], method="fisher")
    tests.append(dict(method=method, regression=regression, predictors=width, lags=lags,
                      entity_tests=rows, statistic=float(fisher.statistic), p_value=float(fisher.pvalue)))

groups, periods = 24, 7
x = rng.normal(size=(groups, periods)) + np.arange(groups)[:, None] * 0.05
y = np.empty_like(x)
for i in range(groups):
    intercept = rng.normal()
    y[i, 0] = rng.normal()
    for t in range(1, periods):
        y[i, t] = intercept + 0.4 * y[i, t - 1] + 0.8 * x[i, t] + rng.normal(scale=0.4)

dy, dx = np.diff(y), np.diff(x)
design = np.stack([dy[:, :-1], dx[:, 1:]], axis=2)
outcomes = dy[:, 1:]
instruments = np.zeros((groups, periods - 2, 3))
instruments[:, :, 0] = y[:, :-2]
instruments[:, 1:, 1] = y[:, :-3]
instruments[:, :, 2] = dx[:, 1:]
difference = np.diff(np.eye(periods - 1), axis=0)
weight_inverse = np.mean([z.T @ difference @ difference.T @ z for z in instruments], axis=0)


class EntityGMM(GMM):
    def momcond(self, params):
        return np.asarray([z.T @ (yi - xi @ params)
                           for z, yi, xi in zip(instruments, outcomes, design)])


model = EntityGMM(np.zeros(groups), np.zeros((groups, 2)), np.zeros((groups, 3)), k_moms=3, k_params=2)
fit = model.fit(start_params=np.zeros(2), maxiter=0, inv_weights=weight_inverse,
                has_optimal_weights=False, weights_method="cov", wargs={"centered": False},
                optim_args={"disp": False, "gtol": 1e-11})
predicted = design @ fit.params
residuals = outcomes - predicted
# For iid level errors, Var(delta error) = sigma^2 D D'.
gradient = np.mean([z.T @ xi for z, xi in zip(instruments, design)], axis=0)
nonrobust = (np.sum(residuals ** 2) / (2 * residuals.size)
             * np.linalg.inv(gradient.T @ np.linalg.solve(weight_inverse, gradient)) / groups)
dynamic = dict(response=y.ravel().tolist(), predictors=[x.ravel().tolist()],
               entity=np.repeat(np.arange(groups, dtype=float), periods).tolist(),
               time=np.tile(np.arange(periods, dtype=float), groups).tolist(),
               coefficients=fit.params.tolist(), covariance=fit.cov_params().tolist(),
               nonrobust_covariance=nonrobust.tolist(),
               p_values=(2 * norm.sf(np.abs(fit.params / fit.bse))).tolist(),
               fitted=predicted.ravel().tolist(), residuals=residuals.ravel().tolist())
result = dict(reference=dict(numpy=np.__version__, scipy=scipy.__version__, statsmodels=statsmodels.__version__),
              nonstationary=dict(response=response, predictors=predictors, entity=entity, time=time, cases=tests),
              dynamic=dynamic)
Path(__file__).with_suffix(".json").write_text(json.dumps(result, indent=2, allow_nan=False) + "\n", encoding="utf-8")
