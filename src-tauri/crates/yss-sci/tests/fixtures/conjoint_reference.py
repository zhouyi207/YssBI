"""Ratings conjoint reference: statsmodels OLS and NumPy linear contrasts."""
import json
from pathlib import Path
import numpy as np
import statsmodels.api as sm

i = np.arange(648)
a = i % 2
b = (i // 2) % 3
y = 3 + 1.4*a + .8*(b == 1) - .5*(b == 2) + ((i*17) % 23-11)*.07
x = np.column_stack([np.ones(len(i)), a, b == 1, b == 2])
fit = sm.OLS(y, x).fit()
c = np.array([[0, -.5, 0, 0], [0, .5, 0, 0],
              [0, 0, -1/3, -1/3], [0, 0, 2/3, -1/3], [0, 0, -1/3, 2/3]])
u = c @ fit.params
ranges = np.array([np.ptp(u[:2]), np.ptp(u[2:])])
out = {
    "utilities": u.tolist(),
    "standard_errors": np.sqrt(np.diag(c @ fit.cov_params() @ c.T)).tolist(),
    "importance": (100*ranges/ranges.sum()).tolist(),
    "intercept": float(np.array([1, .5, 1/3, 1/3]) @ fit.params),
    "r_squared": float(fit.rsquared), "last_fitted": float(fit.fittedvalues[-1]),
    "last_residual": float(fit.resid[-1]),
}
Path(__file__).with_suffix(".json").write_text(json.dumps(out, indent=2)+"\n", encoding="utf-8")
