"""Regenerate binary_postestimation.json with statsmodels 0.14.6.

Run in an isolated Python environment with statsmodels==0.14.6 installed.
The Rust tests consume the checked-in values, not Python at test time.
"""
import json
from pathlib import Path

import numpy as np
import statsmodels
import statsmodels.api as sm

assert statsmodels.__version__ == "0.14.6"
x = np.array([-1.7, -1.2, -.8, -.4, .1, .5, .9, 1.3, 1.7, 2.1, 2.6, 3.2])
z = np.array([.2, 1.4, -.3, 1.2, -.8, .6, 1.8, -1.1, .4, 1.1, -.4, .8])
y = np.array([0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1, 1])
xmat = np.column_stack([np.ones(len(x)), x, z])
output = {
    "source": "statsmodels 0.14.6; independently fitted binary MLE, normal delta-method margins",
    "response": y.tolist(),
    "predictors": [x.tolist(), z.tolist()],
    "models": [],
}
for model_class in [sm.Logit, sm.Probit]:
    fit = model_class(y, xmat).fit(disp=False)
    model = {
        "link": model_class.__name__.lower(),
        "coefficients": fit.params.tolist(),
        "covariance": fit.cov_params().tolist(),
        "effects": [],
    }
    for at in ["overall", "mean"]:
        for method in ["dydx", "eyex", "eydx", "dyex"]:
            for override in [False, True]:
                effects = fit.get_margeff(
                    at=at, method=method, atexog={1: .75} if override else None
                )
                model["effects"].append({
                    "evaluation": at, "method": method, "override": override,
                    "estimate": effects.margeff.tolist(),
                    "se": effects.margeff_se.tolist(),
                    "p": effects.pvalues.tolist(),
                    "ci": effects.conf_int().tolist(),
                })
    output["models"].append(model)
Path(__file__).with_name("binary_postestimation.json").write_text(
    json.dumps(output, indent=2) + "\n"
)
