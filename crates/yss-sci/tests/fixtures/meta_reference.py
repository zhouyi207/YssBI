"""Independent meta-analysis references: NumPy, SciPy and statsmodels.

Run python <this-file>. Pooling uses statsmodels' combine_effects; moderator
models use WLS and SciPy root finding. Hedges g uses the exact gamma correction
and metafor's LS variance, which differs from statsmodels' default SMD variance.
"""
import json
from pathlib import Path

import numpy as np
from scipy import optimize, special, stats
import statsmodels.api as sm
from statsmodels.stats.meta_analysis import combine_effects

y = np.array([-.4, .1, .5, .35, 1.1, .9, -.1, .65])
v = np.array([.04, .09, .16, .0225, .0625, .12, .05, .08])
x = np.array([-2., -1., .5, 1., 2., 3., -.5, 1.4])
level = .9


def pooling(y, v, method, kh):
    fit = combine_effects(y, v, method_re="chi2" if method == "dl" else "iterated",
                          use_t=kh, atol=1e-12)
    fixed = method == "fixed"
    mean = fit.mean_effect_fe if fixed else fit.mean_effect_re
    variance = fit.var_eff_w_fe if fixed else fit.var_eff_w_re
    scale = (fit.scale_hksj_fe if fixed else fit.scale_hksj_re) if kh else 1.
    se = np.sqrt(variance * scale)
    q = stats.t.ppf((1+level)/2, len(y)-1) if kh else stats.norm.ppf((1+level)/2)
    statistic = mean / se
    p = 2 * (stats.t.sf(abs(statistic), len(y)-1) if kh else stats.norm.sf(abs(statistic)))
    return dict(method=method, kh=kh, estimate=mean, se=se, ci=[mean-q*se, mean+q*se],
                p=p, tau=0. if fixed else fit.tau2, q=fit.q,
                i2=max(0., fit.i2)*100, weights=fit.weights_rel_fe if fixed else fit.weights_rel_re)


def regression(method, kh):
    design = sm.add_constant(x)
    initial = sm.WLS(y, design, weights=1/v).fit()
    df = len(y) - design.shape[1]
    tau = 0.
    if method == "dl":
        w = np.diag(1/v)
        residual_precision = w - w @ design @ np.linalg.inv(design.T @ w @ design) @ design.T @ w
        tau = max(0., (initial.ssr-df)/np.trace(residual_precision))
    if method == "pm" and initial.ssr > df:
        tau = optimize.brentq(lambda t: sm.WLS(y, design, weights=1/(v+t)).fit().ssr-df, 0, 10, xtol=1e-14)
    result = sm.WLS(y, design, weights=1/(v+tau)).fit()
    cov = result.normalized_cov_params * (result.scale if kh else 1.)
    return dict(method=method, kh=kh, beta=result.params, covariance=cov, tau=tau,
                q=initial.ssr, residual_q=result.ssr, fitted=result.fittedvalues)


nt, nc = np.array([20., 45., 72.]), np.array([18., 40., 65.])
mt, mc = np.array([6.2, 4.1, 8.8]), np.array([5.3, 4.8, 7.9])
st, sc = np.array([1.2, 2.3, 3.1]), np.array([1.6, 1.9, 2.8])
df = nt+nc-2
j = np.exp(special.gammaln(df/2) - .5*np.log(df/2) - special.gammaln((df-1)/2))
g = j*(mt-mc) / np.sqrt(((nt-1)*st**2+(nc-1)*sc**2)/df)
precision = 1/np.sqrt(v)
egger = sm.OLS(y*precision, sm.add_constant(precision)).fit()
center = np.average(y, weights=1/v)
begg = stats.kendalltau((y-center)/np.sqrt(v-1/(1/v).sum()), v, method="asymptotic")
p_values = np.array([.01, .2, .05, .001])
p_weights = np.array([1., 2., 1.5, 3.])
proportions = np.array([0., 7., 20.])
totals = np.array([15., 30., 20.])
corrected = proportions.copy()
denominator = totals.copy()
boundary = (proportions == 0) | (proportions == totals)
corrected[boundary] += .5
denominator[boundary] += 1
a = np.array([0., 8., 12.])
b = np.array([20., 17., 8.])
c = np.array([4., 5., 6.])
d = np.array([16., 20., 14.])
binary_input = [a.copy(), a+b, c.copy(), c+d]
zero = (np.array([a, b, c, d]) == 0).any(axis=0)
for cells in [a, b, c, d]:
    cells[zero] += .5
binary = {}
for method in ["log_odds_ratio", "log_risk_ratio", "risk_difference"]:
    if method == "log_odds_ratio":
        effect, variance = np.log(a*d/(b*c)), 1/a+1/b+1/c+1/d
    elif method == "log_risk_ratio":
        effect, variance = np.log(a/(a+b)/(c/(c+d))), 1/a-1/(a+b)+1/c-1/(c+d)
    else:
        effect, variance = a/(a+b)-c/(c+d), a*b/(a+b)**3+c*d/(c+d)**3
    binary[method] = dict(effect=effect, variance=variance)

result = dict(
    y=y, variances=v, moderator=x, confidence=level,
    pooling=[pooling(y, v, m, kh) for m in ["fixed", "dl", "pm"] for kh in [False, True]],
    regression=[regression(m, kh) for m in ["fixed", "dl", "pm"] for kh in [False, True]],
    continuous=dict(inputs=[mt, st, nt, mc, sc, nc], md=mt-mc, md_variance=st**2/nt+sc**2/nc,
                    g=g, g_variance=1/nt+1/nc+g**2/(2*(nt+nc))),
    binary_inputs=binary_input, binary=binary,
    proportion=dict(events=proportions, totals=totals, proportion=corrected/denominator,
                    logit=np.log(corrected/(denominator-corrected)),
                    logit_variance=1/corrected+1/(denominator-corrected),
                    arcsine=np.arcsin(np.sqrt(proportions/totals))),
    egger=dict(beta=egger.params, se=egger.bse, p=egger.pvalues),
    begg=dict(tau=begg.statistic, p=begg.pvalue),
    p_values=p_values, p_weights=p_weights,
    fisher=stats.combine_pvalues(p_values, method="fisher"),
    stouffer=stats.combine_pvalues(p_values, method="stouffer", weights=p_weights),
    omissions=[pooling(np.delete(y, i), np.delete(v, i), "pm", False) for i in range(len(y))],
)


def encode(value):
    if isinstance(value, np.ndarray):
        return value.tolist()
    if isinstance(value, np.generic):
        return value.item()
    raise TypeError(type(value))


Path(__file__).with_suffix(".json").write_text(json.dumps(result, default=encode, indent=2, allow_nan=False)+"\n", encoding="utf8")
