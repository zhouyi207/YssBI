"""Independent quadratic OLS and four-parameter curve reference (statsmodels/SciPy)."""
import json
from pathlib import Path
import numpy as np
import statsmodels.api as sm
from scipy.optimize import least_squares
from scipy.stats import t

i = np.arange(640)
x1, x2 = 10 + 2 * (i % 8), 100 + 10 * ((i // 8) % 8)
z1, z2 = (x1 - 17) / 7, (x2 - 135) / 35
y = 50 + 4*z1 - 2*z2 - 3*z1*z1 - 4*z2*z2 + 1.2*z1*z2 + .03*((i*7)%17-8)
fit = sm.OLS(y, np.column_stack([np.ones(640), z1, z2, z1*z1, z2*z2, z1*z2])).fit()
b = fit.params
h = np.array([[2*b[3], b[5]], [b[5], 2*b[4]]])
stationary = -np.linalg.solve(h, b[1:3])
surface = dict(coefficients=b.tolist(), se=fit.bse.tolist(), r2=fit.rsquared,
               rss=np.sum(fit.resid**2), stationary=stationary.tolist(),
               prediction=b[0]+.5*stationary@b[1:3], eigenvalues=np.linalg.eigvalsh(h).tolist(),
               last_fitted=fit.fittedvalues[-1])
dose = np.where(i%32 == 0, 0., np.exp(((i%32)-16)/4))
y = 1.2 + 8.4/(1+(dose/2.5)**1.4) + .03*((i*17)%19-9)
def curve(b):
    return b[0]+(b[1]-b[0])/(1+(dose/np.exp(b[3]))**np.exp(b[2]))
fit = least_squares(lambda b: curve(b)-y, [1,10,np.log(1.5),np.log(2.5)], jac='3-point',
                    ftol=1e-13, xtol=1e-13, gtol=1e-13)
cov = np.linalg.inv(fit.jac.T@fit.jac)*np.sum(fit.fun**2)/(640-4)
se = np.sqrt(np.diag(cov))
q = t.ppf(.975,636)
curve_result = dict(coefficients=fit.x.tolist(),se=se.tolist(),rss=np.sum(fit.fun**2),
                    ed50=np.exp(fit.x[3]),ed50_ci=np.exp(fit.x[3]+np.array([-1,1])*q*se[3]).tolist(),
                    hill=np.exp(fit.x[2]),last_fitted=curve(fit.x)[-1])
Path(__file__).with_suffix('.json').write_text(json.dumps(dict(surface=surface,dose=curve_result),indent=2)+'\n')
