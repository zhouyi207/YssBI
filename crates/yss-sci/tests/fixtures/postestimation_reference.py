"""Statsmodels CR1 and prediction APIs, independent of Rust implementations."""
import json
from pathlib import Path
import numpy as np
import statsmodels.api as sm

rng = np.random.default_rng(123)
n = 640
groups = np.repeat(np.arange(20), 32)
x = rng.normal(size=(n, 2))
y = 1.5 + x @ np.array([0.7, -0.3]) + rng.normal(size=20)[groups] + rng.normal(size=n)*0.4
binary = rng.binomial(1, 1/(1+np.exp(-(-0.4+x@np.array([0.7, -0.5])))))
design = sm.add_constant(x)
cluster = sm.OLS(y, design).fit(cov_type="cluster", cov_kwds={"groups": groups, "use_correction": True}, use_t=True)
result = dict(response=y.tolist(), binary=binary.tolist(), predictors=x.T.tolist(), groups=groups.tolist(),
              cluster=dict(beta=cluster.params.tolist(), covariance=cluster.cov_params().tolist(),
                           p=cluster.pvalues.tolist(), ci=cluster.conf_int().tolist()), predictions=[])
for family in ["linear", "HC3", "logit", "probit"]:
    if family in ["linear", "HC3"]:
        fit = sm.OLS(y, design).fit(cov_type="nonrobust" if family == "linear" else "HC3")
    else:
        fit = (sm.Logit(binary, design) if family == "logit" else sm.Probit(binary, design)).fit(disp=False)
    for evaluation in ["average", "at_means"]:
        for override in [False, True]:
            exog = design.copy() if evaluation == "average" else design.mean(axis=0, keepdims=True)
            if override:
                exog[:, 1] = 0.75
            if family in ["linear", "HC3"]:
                prediction = fit.get_prediction(exog.mean(axis=0, keepdims=True))
                estimate = float(prediction.predicted_mean[0])
                se = float(prediction.se_mean[0])
                ci = prediction.conf_int(alpha=.1)[0].tolist()
            else:
                prediction = fit.get_prediction(exog, which="mean", average=True)
                estimate = float(prediction.predicted)
                se = float(prediction.se)
                ci = np.asarray(prediction.conf_int(alpha=.1)).reshape(-1).tolist()
            result["predictions"].append(dict(family=family, evaluation=evaluation, override=override,
                                              estimate=estimate, se=se, ci=ci))
Path(__file__).with_suffix(".json").write_text(json.dumps(result, indent=2, allow_nan=False)+"\n", encoding="utf8")
