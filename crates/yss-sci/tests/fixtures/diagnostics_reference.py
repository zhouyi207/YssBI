"""Independent references: run python <this-file>; NumPy, SciPy and statsmodels.

Gaussian and binary results use statsmodels. PH score/information is evaluated
by PHReg on split counting-process data with explicit time interactions, without
using the application's event-moment implementation.
"""
import json
from pathlib import Path

import numpy as np
from scipy import stats
import statsmodels.api as sm
from statsmodels.duration.hazard_regression import PHReg
from statsmodels.stats.outliers_influence import OLSInfluence, variance_inflation_factor

rng = np.random.default_rng(92047)
n = 640
x = rng.normal(size=(n, 3))
x[:, 2] += 0.6 * x[:, 0]
y = 1.2 + x @ [0.7, -0.18, 0.12] + rng.normal(size=n)
weights = rng.uniform(0.3, 2, n)
full_x = sm.add_constant(x)
restricted_x = full_x[:, :2]


def ic(fit, gaussian):
    k = len(fit.params) + int(gaussian)
    return dict(parameters=k, log_likelihood=fit.llf,
                aic=-2 * fit.llf + 2 * k, bic=-2 * fit.llf + np.log(n) * k)


gaussian = {}
for name, w in [("ols", np.ones(n)), ("wls", weights)]:
    restricted = sm.WLS(y, restricted_x, weights=w).fit()
    full = sm.WLS(y, full_x, weights=w).fit()
    influence = OLSInfluence(sm.OLS(y * np.sqrt(w), full_x * np.sqrt(w[:, None])).fit())
    gaussian[name] = dict(criteria=ic(full, True),
                          lr=full.compare_lr_test(restricted),
                          lm=full.compare_lm_test(restricted),
                          f=full.compare_f_test(restricted),
                          leverage=influence.hat_matrix_diag,
                          standardized=influence.resid_studentized_internal,
                          studentized=influence.resid_studentized_external,
                          cooks=influence.cooks_distance[0])

binary_y = rng.binomial(1, stats.norm.cdf(-0.2 + x @ [0.5, -0.3, 0.25]))
binary = {}
for name, estimator in [("logit", sm.Logit), ("probit", sm.Probit)]:
    restricted = estimator(binary_y, restricted_x).fit(disp=0, tol=1e-11)
    full = estimator(binary_y, full_x).fit(disp=0, tol=1e-11)
    link = sm.families.links.Logit() if name == "logit" else sm.families.links.Probit()
    score_fit = sm.GLM(binary_y, restricted_x, family=sm.families.Binomial(link=link)).fit()
    lm = score_fit.model.score_test(score_fit.params, exog_extra=x[:, 1:], observed=False)
    binary[name] = dict(criteria=ic(full, False),
                        lr=2 * (full.llf - restricted.llf),
                        lr_p=stats.chi2.sf(2 * (full.llf - restricted.llf), 2),
                        lm=float(np.asarray(lm[0]).item()), lm_p=float(np.asarray(lm[1]).item()))

standardized = (x - x.mean(axis=0)) / x.std(axis=0, ddof=1)
design = np.column_stack([np.ones(n), standardized])
scaled = design / np.sqrt((design * design).sum(axis=0))
eigenvalues = np.linalg.eigvalsh(scaled.T @ scaled)[::-1]
collinearity = dict(eigenvalues=eigenvalues, condition=np.linalg.cond(scaled),
                    vif=[variance_inflation_factor(design, i) for i in range(1, 4)])
harman_values = np.linalg.eigvalsh(np.corrcoef(x.T))[::-1]
harman = dict(eigenvalues=harman_values, ratio=harman_values / harman_values.sum())

survival = json.loads(Path(__file__).with_name("survival_category_reference.json").read_text())
data = survival["data"]
time = np.asarray(data["time"])
event = np.asarray(data["event"])
sx = np.asarray(data["predictors"]).T
event_times = np.unique(time[event == 1])
ph = []
for ties in ["efron", "breslow"]:
    fit = PHReg(time, sx, status=event, ties=ties).fit(disp=0)
    for transform in ["rank", "log", "identity"]:
        def g(t):
            if transform == "log":
                return np.log(t)
            if transform == "identity":
                return t
            return np.searchsorted(np.sort(time), t, side="left") / 2 + np.searchsorted(np.sort(time), t, side="right") / 2 + 0.5

        starts, stops, events, columns = [], [], [], []
        for i, end in enumerate(time):
            start = 0.0
            for stop in np.r_[event_times[event_times < end], end]:
                starts.append(np.nextafter(start, np.inf))
                stops.append(stop)
                events.append(event[i] if stop == end else 0)
                columns.append(np.r_[sx[i], sx[i] * g(stop)])
                start = stop
        extended = PHReg(np.asarray(stops), np.asarray(columns), status=np.asarray(events),
                         entry=np.asarray(starts), ties=ties)
        theta = np.r_[fit.params, np.zeros(sx.shape[1])]
        score = extended.score(theta)
        information = -extended.hessian(theta)
        p = sx.shape[1]
        correction = information[p:, :p] @ np.linalg.inv(information[:p, :p])
        efficient = information[p:, p:] - correction @ information[:p, p:]
        u = score[p:] - correction @ score[:p]
        terms = u**2 / np.diag(efficient)
        global_test = u @ np.linalg.solve(efficient, u)
        ph.append(dict(ties=ties, transform=transform, terms=terms,
                       terms_p=stats.chi2.sf(terms, 1), global_statistic=global_test,
                       global_p=stats.chi2.sf(global_test, p)))


def json_value(value):
    if isinstance(value, np.ndarray):
        return value.tolist()
    if isinstance(value, np.generic):
        return value.item()
    raise TypeError(type(value).__name__)


output = dict(x=x.T, y=y, weights=weights, binary_y=binary_y,
              gaussian=gaussian, binary=binary, collinearity=collinearity,
              harman=harman, survival=data, ph=ph)
Path(__file__).with_suffix(".json").write_text(json.dumps(output, default=json_value, indent=2, allow_nan=False) + "\n")
