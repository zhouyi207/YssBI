"""Independent NumPy/SciPy references; run this file to regenerate the JSON.

Uses dense determinants, bounded scalar/global optimizers, analytic observed
information and SciPy's orthonormal Helmert transform. Does not import project code.
Model definitions: https://r-spatial.github.io/spatialreg/reference/ML_models.html
Moran moments: https://pysal.org/esda/stable/user-guide/global_morans_i.html
"""
import json
from pathlib import Path
import numpy as np
import scipy
from scipy import linalg, optimize, stats

rng = np.random.default_rng(37143)
n = 48
coords = rng.uniform(0, 8, (n, 2))
d = linalg.norm(coords[:, None, :] - coords[None, :, :], axis=2)
np.fill_diagonal(d, np.inf)
w = np.zeros((n, n))
w[np.arange(n)[:, None], np.argsort(d, axis=1)[:, :4]] = 0.25
x = rng.normal(size=(n, 2))
eye = np.eye(n)
y = linalg.solve(eye - 0.35*w, 1.2 + x @ [0.85, -0.55]
    + linalg.solve(eye - 0.2*w, rng.normal(0, 0.6, n)))

def fit(method, y, x, w, constant=True):
    lag_y = method in ['slm', 'sac', 'sdm']
    lag_error = method in ['sem', 'sac', 'sdem']
    lag_x = method in ['sdm', 'sdem', 'slx']
    count = x.shape[1]
    z = np.column_stack([x, w @ x]) if lag_x else x
    if constant:
        z = np.column_stack([np.ones(len(y)), z])
    p = z.shape[1]
    size = len(y)
    eye = np.eye(size)
    def unpack(a):
        return (a[0] if lag_y else 0.0, a[int(lag_y)] if lag_error else 0.0)
    def objective(a, details=False):
        rho, lam = unpack(a)
        a, b = eye-rho*w, eye-lam*w
        yy, xx = b @ (a @ y), b @ z
        beta = linalg.lstsq(xx, yy)[0]
        e = yy-xx @ beta
        sig2 = e@e/size
        nll = size/2*(np.log(2*np.pi)+1+np.log(sig2)) - np.linalg.slogdet(a)[1]-np.linalg.slogdet(b)[1]
        return (nll, beta, e, sig2, a, b) if details else nll
    s = int(lag_y) + int(lag_error)
    if s == 1:
        r = optimize.minimize_scalar(lambda t: objective([t]), bounds=(-0.9999, 0.9999),
            method='bounded', options={'xatol': 1e-13})
        pars = np.array([r.x])
    elif s == 2:
        r = optimize.differential_evolution(objective, [(-0.9999, 0.9999)]*2,
            rng=np.random.default_rng(1701), tol=1e-11, polish=True)
        pars = r.x
    else:
        pars = np.array([])
    assert all(abs(pars) < 0.99), (method, pars)
    rho, lam = unpack(pars)
    nll, beta, e, sig2, a, b = objective(pars, True)
    if s:
        # Natural beta/rho/lambda/log(sigma^2) axes, analytic observed Hessian.
        gradients = [-b@z[:, j] for j in range(p)]
        if lag_y: gradients.append(-b@w@y)
        if lag_error: gradients.append(-w@(a@y-z@beta))
        g = np.column_stack(gradients)
        h = np.zeros((p+s+1, p+s+1))
        h[:-1, :-1] = g.T@g/sig2
        if lag_y:
            aw = linalg.solve(a, w)
            h[p, p] += np.trace(aw@aw)
        if lag_error:
            j = p+int(lag_y)
            bw = linalg.solve(b, w)
            h[j, j] += np.trace(bw@bw)
            h[:p, j] += e@(w@z)/sig2
            h[j, :p] = h[:p, j]
            if lag_y:
                h[p, j] += e@(w@w@y)/sig2
                h[j, p] = h[p, j]
        h[:-1, -1] = -g.T@e/sig2
        h[-1, :-1] = h[:-1, -1]
        h[-1, -1] = e@e/(2*sig2)
        assert np.linalg.eigvalsh(h).min() > 0, (method, pars)
        covariance = linalg.inv(h)[:-1, :-1]
    else:
        sig2 = e@e/(size-p)
        covariance = linalg.inv(z.T@z)*sig2
    coefficients = np.r_[beta, pars]
    conditional = z@beta + rho*w@y
    inv = linalg.inv(a)
    impacts = []
    for k in range(count):
        theta = beta[int(constant)+count+k] if lag_x else 0.0
        effect = inv@(eye*beta[int(constant)+k]+theta*w)
        direct, total = np.trace(effect)/size, effect.sum()/size
        impacts.append([direct, total-direct, total])
    return dict(coefficients=coefficients.tolist(), covariance=covariance.tolist(),
        log_likelihood=-nll, sigma_squared=sig2, fitted=conditional.tolist(),
        innovations=e.tolist(), reduced_fitted=(inv@z@beta).tolist(), impacts=impacts)

def moran(y, w):
    n = len(y)
    z = y-y.mean()
    s0 = w.sum()
    s1 = ((w+w.T)**2).sum()/2
    s2 = ((w.sum(axis=0)+w.sum(axis=1))**2).sum()
    ei = -1/(n-1)
    value = n/s0*(z@w@z)/(z@z)
    vn = (n*n*s1-n*s2+3*s0*s0)/((n*n-1)*s0*s0)-ei*ei
    kurt = n*(z**4).sum()/(z@z)**2
    vr = (n*((n*n-3*n+3)*s1-n*s2+3*s0*s0)-kurt*((n*n-n)*s1-2*n*s2+6*s0*s0))/((n-1)*(n-2)*(n-3)*s0*s0)-ei*ei
    return dict(statistic=value, expected=ei, normal_variance=vn, randomization_variance=vr,
        normal_p_value=2*stats.norm.sf(abs(value-ei)/np.sqrt(vn)),
        randomization_p_value=2*stats.norm.sf(abs(value-ei)/np.sqrt(vr)))

models = {m: fit(m, y, x, w) for m in ['ols', 'slx', 'slm', 'sem', 'sac', 'sdm', 'sdem']}
t = 5
px = rng.normal(size=(t*n, 2)) + np.tile(x, (t, 1))
effects = rng.normal(1, 0.7, n)
py = np.concatenate([linalg.solve(eye-0.3*w, effects+px[k*n:(k+1)*n]@[0.8,-0.5]
    + linalg.solve(eye-0.2*w, rng.normal(0,0.6,n))) for k in range(t)])
transform = np.kron(linalg.helmert(t), np.eye(n))
panel_w = np.kron(np.eye(t-1), w)
panels = {m: fit(m, transform@py, transform@px, panel_w, False) for m in ['slm','sem']}
data = dict(versions=dict(numpy=np.__version__, scipy=scipy.__version__),
    data=dict(x=coords[:,0].tolist(), y_coordinate=coords[:,1].tolist(), weights=w.tolist(),
        response=y.tolist(), predictors=x.T.tolist()),
    moran=moran(y,w), models=models, panel=dict(periods=t, response=py.tolist(), predictors=px.T.tolist(), models=panels))
Path(__file__).with_suffix('.json').write_text(json.dumps(data, indent=2, allow_nan=False)+'\n', encoding='utf-8')
print('Wrote spatial_category_reference.json')
