"""Independent statsmodels/SciPy references. Run directly to regenerate reference.json.
Then format the generated file with the repository-root pnpm format:ts script.

GEE: statsmodels.org/stable/gee.html; LMM: statsmodels MixedLM.
GLMM: lme4.github.io/lme4/reference/glmer.html, one-dimensional Laplace ML;
SciPy bracketed scalar modes and BFGS differ from the Rust Newton/BFGS implementation.
"""
import json
from pathlib import Path
import numpy as np
import scipy
from scipy.optimize import minimize, brentq
from scipy.special import expit, gammaln
import statsmodels
import statsmodels.api as sm
import pandas as pd
from statsmodels.tools.numdiff import approx_hess

rng = np.random.default_rng(20261001)
groups = np.repeat(np.arange(18), 8)
n = len(groups)
x = rng.normal(size=n) * 1.3 + 0.4
X = sm.add_constant(x)
random = rng.normal(size=18)
y = 1.3 + 0.65*x + random[groups] + rng.normal(scale=0.5, size=n)
data = {"source": {"seed": 20261001, "scipy": scipy.__version__, "statsmodels": statsmodels.__version__}, "x": [x.tolist()], "groups": groups.tolist(), "cases": {}}

def record(name, response, beta, se, fitted, **kwargs):
    data["cases"][name] = {"y": response.tolist(), "coefficients": np.asarray(beta).tolist(), "standard_errors": np.asarray(se).tolist(), "fitted": np.asarray(fitted).tolist(), **kwargs}

for reml in [False, True]:
    fit = sm.MixedLM(y, X, groups).fit(reml=reml, method="bfgs", gtol=1e-10, maxiter=1000)
    record("lmm_reml" if reml else "lmm_ml", y, fit.fe_params, np.sqrt(np.diag(fit.cov_params())[:2]), fit.fittedvalues, scale=fit.scale, variance=fit.cov_re[0,0], log_likelihood=fit.llf)
    # Rust uses the conditional GLS covariance, the usual lme4 LMM Wald covariance.
    V = np.eye(n)*fit.scale + (groups[:,None] == groups[None,:])*fit.cov_re[0,0]
    data["cases"]["lmm_reml" if reml else "lmm_ml"]["standard_errors"] = np.sqrt(np.diag(np.linalg.inv(X.T @ np.linalg.solve(V,X)))).tolist()

binary = rng.binomial(1, expit(-0.4+0.65*x+1.1*random[groups]))
poisson = rng.poisson(np.exp(0.35+0.3*x+0.6*random[groups]))
nb = rng.negative_binomial(1/0.55, 1/(1+0.55*np.exp(0.4+0.3*x+0.7*random[groups])))
for name, response, family, correlation in [
    ("gee_gaussian", y, sm.families.Gaussian(), sm.cov_struct.Exchangeable()),
    ("gee_binomial", binary, sm.families.Binomial(), sm.cov_struct.Independence()),
    ("gee_poisson", poisson, sm.families.Poisson(), sm.cov_struct.Exchangeable()),
]:
    fit = sm.GEE(response, X, groups, family=family, cov_struct=correlation).fit(maxiter=1000, ctol=1e-10)
    record(name,response,fit.params,fit.bse,fit.fittedvalues,correlation=float(correlation.dep_params or 0.0),scale=fit.scale)

# Diagonal random-intercept/slope covariance, estimated independently by MixedLM.
slope_y = y + rng.normal(scale=0.65,size=18)[groups]*x
free = sm.regression.mixed_linear_model.MixedLMParams.from_components(fe_params=np.ones(2),cov_re=np.eye(2))
fit = sm.MixedLM(slope_y,X,groups,exog_re=X).fit(reml=True,free=free,method="bfgs",gtol=1e-9,maxiter=1000)
V = np.eye(n)*fit.scale + (groups[:,None]==groups[None,:])*(fit.cov_re[0,0]+fit.cov_re[1,1]*x[:,None]*x[None,:])
record("random_slope",slope_y,fit.fe_params,np.sqrt(np.diag(np.linalg.inv(X.T@np.linalg.solve(V,X)))),fit.fittedvalues,scale=fit.scale,variances=np.diag(fit.cov_re).tolist(),log_likelihood=fit.llf)

for name, second in [("nested", groups//3),("crossed",np.tile(np.arange(8),18))]:
    response=y+rng.normal(scale=0.9,size=int(second.max()+1))[second]
    table=pd.DataFrame({"y":response,"x":x,"g":groups,"h":second,"all":0})
    fit=sm.MixedLM.from_formula("y ~ x",groups="all",re_formula="0",vc_formula={"g":"0+C(g)","h":"0+C(h)"},data=table).fit(reml=True,method="bfgs",gtol=1e-9,maxiter=1000)
    V=np.eye(n)*fit.scale+(groups[:,None]==groups[None,:])*fit.vcomp[0]+(second[:,None]==second[None,:])*fit.vcomp[1]
    record(name,response,fit.fe_params,np.sqrt(np.diag(np.linalg.inv(X.T@np.linalg.solve(V,X)))),fit.fittedvalues,scale=fit.scale,variances=fit.vcomp.tolist(),log_likelihood=fit.llf,second_groups=second.tolist())

for name,response in [("glmm_binomial",binary),("glmm_poisson",poisson),("glmm_nb",nb)]:
    def calc(par, details=False):
        sd=np.exp(par[2]); a=np.exp(par[3]) if name=="glmm_nb" else 0.0
        eta=X@par[:2]; total=0.; fitted=np.zeros(n)
        for g in range(18):
            idx=groups==g; yy=response[idx]; ee=eta[idx]
            def score(u):
                m=expit(ee+sd*u) if name=="glmm_binomial" else np.exp(ee+sd*u)
                return sd*np.sum((yy-m)/(1+a*m))-u
            u=brentq(score,-50,50,xtol=1e-13)
            e=ee+sd*u
            if name=="glmm_binomial":
                m=expit(e); value=np.sum(np.logaddexp(0,e)-yy*e); w=m*(1-m)
            elif name=="glmm_poisson":
                m=np.exp(e); value=np.sum(m-yy*e+gammaln(yy+1)); w=m
            else:
                m=np.exp(e); k=1/a
                value=-np.sum(gammaln(yy+k)-gammaln(k)-gammaln(yy+1)+k*np.log(k/(k+m))+yy*np.log(m/(k+m)))
                w=m*(1+a*yy)/(1+a*m)**2
            total+=value+u*u/2+np.log1p(sd*sd*np.sum(w))/2
            fitted[idx]=m
        return (total,fitted) if details else total
    start=[0,0.3,-0.3]+([-0.5] if name=="glmm_nb" else [])
    fit=minimize(calc,start,method="BFGS",options={"gtol":1e-7,"maxiter":2000})
    assert np.max(np.abs(fit.jac))<2e-4,(name,fit.message,fit.jac)
    value,fitted=calc(fit.x,True)
    covariance=np.linalg.inv(approx_hess(fit.x,calc))
    record(name,response,fit.x[:2],np.sqrt(np.diag(covariance)[:2]),fitted,variance=float(np.exp(2*fit.x[2])),log_likelihood=-value,alpha=float(np.exp(fit.x[3])) if name=="glmm_nb" else None)

Path(__file__).with_name("reference.json").write_text(json.dumps(data,indent=2,allow_nan=False)+"\n",encoding="utf-8")
print("Generated",len(data["cases"]),"independent longitudinal reference cases")
