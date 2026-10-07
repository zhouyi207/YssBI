"""Rebuild deterministic independent references: python <this-file>.

PHReg supplies Cox partial likelihood/information; SciPy distribution logpdf/logsf
and numerical differentiation supply AFT likelihoods/information. No project code
is used. Entry is moved one ULP right to enforce the (start, stop] convention.
"""
import json
from pathlib import Path
import numpy as np
import scipy
from scipy import optimize, stats
from scipy.differentiate import hessian
import statsmodels
from statsmodels.duration.hazard_regression import PHReg
from statsmodels.duration.survfunc import SurvfuncRight, CumIncidenceRight, survdiff

rng = np.random.default_rng(7193)
n = 120
x = np.column_stack([rng.normal(size=n), rng.uniform(-1, 1, n)])
group = np.arange(n) % 2
treatment = (np.arange(n) // 2) % 2
latent = np.exp(1.5 + x @ [0.35, -0.2] + treatment * (0.15 + 0.3 * group)) * rng.weibull(1.6, n)
censor = rng.uniform(3, 10, n)
time = np.ceil(np.minimum(latent, censor) * 2) / 2
event = (latent <= censor).astype(int)
data = dict(time=time, event=event, predictors=x.T, group=group, treatment=treatment)


def cox(t, e, design, **kwargs):
    fit = PHReg(t, design, status=e, **kwargs).fit(disp=0)
    return dict(coefficients=fit.params, covariance=fit.cov_params(), log_likelihood=fit.llf)


cox_cases = {ties: cox(time, event, x, ties=ties) for ties in ["efron", "breslow"]}
beta = cox_cases["efron"]["coefficients"]
lp = (x - x.mean(axis=0)) @ beta
event_times = np.unique(time[event == 1])
baseline = np.cumsum([np.sum((time == t) & (event == 1)) / np.exp(lp[time >= t]).sum() for t in event_times])
cox_cases["efron"].update(baseline=baseline, baseline_times=event_times,
                        risk=-np.expm1(-baseline[event_times <= 2][-1] * np.exp(lp)))
start = np.concatenate([np.zeros(n), time * 0.413])
stop = np.concatenate([time * 0.413, time])
td_event = np.concatenate([np.zeros(n), event])
td_x = np.vstack([x + [0.2, -0.15], x - [0.1, 0.25]])
td = dict(start=start, stop=stop, event=td_event, predictors=td_x.T,
          subjects=np.tile(np.arange(n), 2),
          reference=cox(stop, td_event, td_x, entry=np.nextafter(start, np.inf), ties="efron"))
sub_x = np.column_stack([treatment * (group == 0), treatment * (group == 1), x[:, 0]])
subgroup = cox(time, event, sub_x, strata=group, ties="efron")
delta = subgroup["coefficients"][1] - subgroup["coefficients"][0]
v = subgroup["covariance"]
subgroup["statistic"] = delta**2 / (v[0, 0] + v[1, 1] - 2 * v[0, 1])

design = np.column_stack([np.ones(n), x])
aft = {}
for family in ["exponential", "weibull", "lognormal", "loglogistic"]:
    p = design.shape[1]

    def distribution(theta):
        scale = np.exp(design @ theta[:p])
        sigma = np.exp(theta[p]) if family != "exponential" else 1
        if family == "exponential":
            return stats.expon(scale=scale)
        if family == "weibull":
            return stats.weibull_min(1 / sigma, scale=scale)
        if family == "lognormal":
            return stats.lognorm(sigma, scale=scale)
        return stats.fisk(1 / sigma, scale=scale)

    def nll(theta):
        d = distribution(theta)
        return -np.where(event == 1, d.logpdf(time), d.logsf(time)).sum()

    initial = np.linalg.lstsq(design, np.log(time), rcond=None)[0]
    if family != "exponential":
        initial = np.r_[initial, 0.0]
    fit = optimize.minimize(nll, initial, method="BFGS", options={"gtol": 1e-8})
    # SciPy's roundoff warning is allowed only after independent score verification.
    score = optimize._numdiff.approx_derivative(lambda t: np.array([nll(t)]), fit.x, method="3-point")
    assert np.max(np.abs(score)) < 2e-4
    def vectorized(theta):
        flat = theta.reshape(len(fit.x), -1)
        result = np.array([nll(flat[:, i]) for i in range(flat.shape[1])])
        return result.reshape(theta.shape[1:])
    information = hessian(vectorized, fit.x).ddf
    aft[family] = dict(coefficients=fit.x[:p], scale=np.exp(fit.x[p]) if len(fit.x)>p else 1,
                       covariance=np.linalg.inv(information), log_likelihood=-fit.fun,
                       median=distribution(fit.x).median(), risk=distribution(fit.x).cdf(2))

small_time = np.array([1, 2, 2, 3, 4, 4, 5, 6.])
small_event = np.array([1, 1, 0, 1, 0, 1, 1, 0])
small_group = np.array([0, 0, 1, 1, 0, 1, 0, 1])
sf = SurvfuncRight(small_time, small_event)
ci = CumIncidenceRight(small_time, np.array([1, 2, 0, 1, 0, 2, 1, 0]))
nonparametric = dict(time=small_time, event=small_event, group=small_group,
                    km_times=sf.surv_times, km=sf.surv_prob, km_se=sf.surv_prob_se,
                    logrank=survdiff(small_time, small_event, small_group),
                    cif_times=ci.times, cif=ci.cinc)


def encode(value):
    if isinstance(value, np.ndarray):
        return value.tolist()
    if isinstance(value, np.generic):
        return value.item()
    raise TypeError(type(value))


result = dict(versions=dict(numpy=np.__version__, scipy=scipy.__version__, statsmodels=statsmodels.__version__),
              data=data, cox=cox_cases, time_dependent=td, subgroup=subgroup, aft=aft, nonparametric=nonparametric)
Path(__file__).with_suffix(".json").write_text(json.dumps(result, default=encode, indent=2, allow_nan=False)+"\n", encoding="utf-8")
print("Wrote survival references", result["versions"])
