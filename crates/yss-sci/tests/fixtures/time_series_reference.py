"""Independent reference generation; requires numpy, scipy, statsmodels and arch.

Run with Python 3 and these optional development-only packages on PYTHONPATH.
The Rust build reads the checked-in JSON and does not need Python.
"""
import json
from pathlib import Path
import numpy as np
import scipy
from scipy.optimize import least_squares
from scipy.signal import lfilter
from scipy.stats import norm
import statsmodels
from statsmodels.regression.linear_model import OLS
from statsmodels.tsa.stattools import kpss
from statsmodels.tsa.exponential_smoothing.ets import ETSModel
from statsmodels.tsa.holtwinters import ExponentialSmoothing
import arch
from arch.unitroot import PhillipsPerron
from arch.univariate import arch_model

rng = np.random.default_rng(8127)
n = 640
innovations = rng.normal(size=n)
w = lfilter([1, 0.3], [1, -0.55], innovations) + 0.2
y = 30 + np.cumsum(w)
result = {"versions": {"scipy": scipy.__version__, "statsmodels": statsmodels.__version__, "arch": arch.__version__}, "series": y.tolist()}
result["stationarity"] = []
for trend in ["n", "c", "ct"]:
    test = PhillipsPerron(y, lags=4, trend=trend)
    result["stationarity"].append({"method": "pp", "trend": trend, "statistic": test.stat, "p_value": test.pvalue})
for trend in ["c", "ct"]:
    stat, p, _, _ = kpss(y, regression=trend, nlags=4)
    result["stationarity"].append({"method": "kpss", "trend": trend, "statistic": stat, "p_value": p})

result["arima"] = []
for ordinary, seasonal in [((1,1,1),(0,0,0,1)), ((0,1,0),(0,0,0,1)), ((0,1,0),(1,1,0,4))]:
    p,d,q = ordinary
    sp,sd,sq,m = seasonal
    diff = np.array([1.])
    for _ in range(d): diff = np.convolve(diff, [1,-1])
    for _ in range(sd): diff = np.convolve(diff, [1]+[0]*(m-1)+[-1])
    offset = len(diff)-1
    transformed = lfilter(diff, [1], y)[offset:]
    burn = max(p+sp*m,q+sq*m)
    def polynomials(b):
        a = np.array([1,-b[0]]) if p else np.array([1.])
        c = np.array([1,b[p]]) if q else np.array([1.])
        if sp: a = np.convolve(a, [1]+[0]*(m-1)+[-b[p+q]])
        return a,c
    def errors(b):
        a,c = polynomials(b)
        numerator = lfilter(a,[1],transformed-b[-1])
        return lfilter([1],c,numerator[burn:])
    count = p+q+sp+1
    initial = np.zeros(count); initial[-1] = transformed.mean()
    fit = least_squares(errors,initial,bounds=([-0.999]*(count-1)+[-np.inf],[0.999]*(count-1)+[np.inf]),xtol=1e-13,ftol=1e-13,gtol=1e-11)
    a,c=polynomials(fit.x)
    resid=np.r_[np.zeros(burn),errors(fit.x)]
    history=list(transformed); residual_history=list(resid); levels=list(y); forecasts=[]
    for _ in range(6):
        f=fit.x[-1]-sum(a[j]*(history[-j]-fit.x[-1]) for j in range(1,len(a)))+sum(c[j]*residual_history[-j] for j in range(1,len(c)))
        history.append(f);residual_history.append(0)
        value=f-sum(diff[j]*levels[-j] for j in range(1,len(diff)))
        levels.append(value);forecasts.append(value)
    impulse = lfilter(c,np.convolve(a,diff),[1.]+[0.]*5)
    variance=np.mean(errors(fit.x)**2)
    margin=norm.ppf(.975)*np.sqrt(variance*np.cumsum(impulse**2))
    result["arima"].append({"order":ordinary,"seasonal":seasonal,"coefficients":fit.x.tolist(),"variance":variance,"forecasts":forecasts,"lower":(np.array(forecasts)-margin).tolist(),"first_row":offset+burn})

smoothing_y = 35 + .13*np.arange(80) + np.tile([-3,1,3,-1],20) + rng.normal(0,.4,80)
result["smoothing_series"] = smoothing_y.tolist()
result["smoothing"] = []
for trend,seasonal,damped in [(False,None,False),(True,None,True),(True,"add",False),(True,"mul",True)]:
    m=4 if seasonal else 1
    initial_level=smoothing_y[:m].mean() if seasonal else smoothing_y[0]
    initial_trend=((smoothing_y[m:2*m].mean()-initial_level)/m if seasonal else smoothing_y[1]-smoothing_y[0]) if trend else 0.
    if seasonal: initial_level+=initial_trend*(m-1)/2
    baseline=initial_level+initial_trend*(np.arange(m)-(m-1))
    initial_seasons=(smoothing_y[:m]-baseline if seasonal=="add" else smoothing_y[:m]/baseline) if seasonal else None
    model=ETSModel(smoothing_y[m:],error="add",trend="add" if trend else None,damped_trend=damped,seasonal=seasonal,seasonal_periods=m if seasonal else None,initialization_method="known",initial_level=initial_level,initial_trend=initial_trend if trend else None,initial_seasonal=initial_seasons)
    params=[.3]+([.03] if trend else [])+([.15] if seasonal else [])+([.95] if damped else [])
    fit=model.smooth(params)
    forecasts=fit.forecast(6)
    if seasonal == "mul":
        model=ExponentialSmoothing(smoothing_y[m:],trend="add",damped_trend=damped,seasonal="mul",seasonal_periods=m,initialization_method="known",initial_level=initial_level,initial_trend=initial_trend,initial_seasonal=initial_seasons)
        fit=model.fit(smoothing_level=.3,smoothing_trend=.1,smoothing_seasonal=.15,damping_trend=.95,optimized=False,remove_bias=False)
        # Use all final updated seasonal states. HW 0.14.6's forecast indexing
        # reuses the previous cycle's last seasonal state at horizons m, 2m, ... .
        base=fit.level[-1]+np.cumsum(.95**np.arange(1,7))*fit.trend[-1]
        forecasts=base*np.resize(fit.season[-m:],6)
    result["smoothing"].append({"trend":trend,"seasonal":seasonal,"damped":damped,"fitted":fit.fittedvalues.tolist(),"forecasts":forecasts.tolist()})

x=np.cumsum(rng.normal(size=n))+10
equilibrium=lfilter([1],[1,-.5],rng.normal(size=n))
response=2+1.4*x+equilibrium
long=OLS(response,np.column_stack([np.ones(n),x])).fit()
start=2
short_x=np.column_stack([np.ones(n-start),long.resid[start-1:-1],np.diff(response)[start-2:-1],np.diff(x)[start-1:],np.diff(x)[start-2:-1]])
short=OLS(np.diff(response)[start-1:],short_x).fit()
result["ecm"]={"y":response.tolist(),"x":x.tolist(),"long":long.params.tolist(),"short":short.params.tolist(),"short_se":short.bse.tolist()}
grey=12*np.exp(.035*np.arange(24))+rng.normal(0,.08,24)
ago=np.cumsum(grey)
grey_fit=OLS(grey[1:],np.column_stack([np.ones(23),-(ago[1:]+ago[:-1])/2])).fit()
b,a=grey_fit.params
accumulated=(grey[0]-b/a)*np.exp(-a*np.arange(30))+b/a
restored=np.diff(accumulated)
result["grey"]={"y":grey.tolist(),"a":a,"b":b,"forecasts":restored[23:29].tolist()}

result["volatility"]=[]
for method in ["arch","garch","egarch","gjr_garch"]:
    shocks=rng.normal(size=1600);v=np.ones(1600);resid=np.zeros(1600)
    for t in range(1,1600):
        if method=="egarch":
            z=resid[t-1]/np.sqrt(v[t-1])
            v[t]=np.exp(.05+.35*(abs(z)-np.sqrt(2/np.pi))-.18*z+.75*np.log(v[t-1]))
        else:
            v[t]=.25+.3*resid[t-1]**2+(0.2*resid[t-1]**2*(resid[t-1]<0) if method=="gjr_garch" else 0)+(0 if method=="arch" else .4*v[t-1])
        resid[t]=np.sqrt(v[t])*shocks[t]
    sample=resid[320:]
    q=0 if method=="arch" else 1
    o=1 if method in ["egarch","gjr_garch"] else 0
    model=arch_model(sample,mean="Zero",vol="EGARCH" if method=="egarch" else "GARCH",p=1,o=o,q=q,rescale=False)
    fit=model.fit(disp="off",backcast=np.var(sample),tol=1e-10,options={"maxiter":1500})
    assert fit.convergence_flag==0
    forecasts=fit.forecast(horizon=1 if method=="egarch" else 6,method="analytic").variance.values[-1]
    result["volatility"].append({"method":method,"y":sample.tolist(),"parameters":fit.params.tolist(),"log_likelihood":fit.loglikelihood,"variances":(fit.conditional_volatility**2).tolist(),"forecasts":forecasts.tolist()})

Path(__file__).with_suffix(".json").write_text(json.dumps(result,ensure_ascii=False,indent=2,allow_nan=False)+"\n",encoding="utf-8")
