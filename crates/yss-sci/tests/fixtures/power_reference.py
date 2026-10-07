"""SciPy noncentral distributions and planning equations; minimum integer sizes by bracketing."""
import json
from pathlib import Path
import numpy as np
from scipy.stats import norm,t,nct,ncf,f,chi2,chi
from scipy.integrate import quad

def normal_power(d,alpha=.05):
    q=norm.isf(alpha/2)
    return norm.sf(q-d)+norm.cdf(-q-d)
def tp(ncp,df,alpha=.05):
    q=t.isf(alpha/2,df)
    value=nct.sf(q,df,ncp)+nct.cdf(-q,df,ncp)
    if np.isfinite(value):
        return value
    # Some SciPy/Boost builds return NaN for an extremely small opposite tail.
    # Independently integrate the defining normal / chi ratio in that case.
    lo=np.sqrt(chi2.ppf(1e-13,df));hi=np.sqrt(chi2.isf(1e-13,df))
    return quad(lambda s:(norm.sf(q*s/np.sqrt(df)-ncp)+norm.cdf(-q*s/np.sqrt(df)-ncp))*chi.pdf(s,df),lo,hi,epsabs=1e-12)[0]
def fp(df1,df2,ncp,alpha=.05):
    return ncf.sf(f.isf(alpha,df1,df2),df1,df2,ncp)
def prop(delta,v0,v1,n):
    q=norm.isf(.025)*np.sqrt(v0/n)
    return norm.sf((q-delta)/np.sqrt(v1/n))+norm.cdf((-q-delta)/np.sqrt(v1/n))
def variance(n):
    df=n-1
    return chi2.cdf(chi2.ppf(.025,df)/1.5,df)+chi2.sf(chi2.isf(.025,df)/1.5,df)
def equivalence(n):
    sd=np.sqrt(2/n)
    a=(-.3-.05)/sd+norm.isf(.05)
    b=(.3-.05)/sd-norm.isf(.05)
    return max(0,norm.cdf(b)-norm.cdf(a))
models={
 'normal':(1,lambda n:normal_power(.5*np.sqrt(n))),
 't_one':(2,lambda n:tp(.5*np.sqrt(n),n-1)),
 't_two':(2,lambda n:tp(.5*np.sqrt(n/2),2*n-2)),
 'variance':(2,variance),
 'proportion':(1,lambda n:prop(.1,.25,.24,n)),
 'proportion_difference':(1,lambda n:prop(.2,.5,.48,n)),
 'correlation':(4,lambda n:normal_power(np.arctanh(.3)*np.sqrt(n-3))),
 'anova':(2,lambda n:fp(2,3*(n-1),3*n*.25**2)),
 'linear':(5,lambda n:fp(3,n-4,n*.15)),
 'poisson':(1,lambda n:normal_power(np.log(1.5)*np.sqrt(n/(1+1/1.5)))),
 'logistic':(1,lambda n:normal_power(np.log(1.5)*np.sqrt(n/(1/(.2*.8)+1/((.3/1.1)*(1-.3/1.1)))))),
 'survival':(1,lambda n:normal_power(np.log(.7)*np.sqrt(n*.5*.25))),
 'cluster':(2,lambda n:tp(.5*np.sqrt(n*20/(2*(1+19*.05))),2*n-2)),
 'noninferiority':(1,lambda n:norm.sf(norm.isf(.05)-.3*np.sqrt(n/2))),
 'equivalence':(1,equivalence),
}
result={}
for name,(minimum,power) in models.items():
    lo=minimum;hi=minimum
    while power(hi)<.8:lo=hi;hi*=2
    while hi-lo>1:
        mid=(lo+hi)//2
        if power(mid)>=.8:hi=mid
        else:lo=mid
    required=minimum if power(minimum)>=.8 else hi
    result[name]=dict(power=power(100),required=required,achieved=power(required))
result['noncentral_t']=[dict(t=q,df=df,delta=d,tail=nct.sf(q,df,d)) for df in [1,2,10,1000,1e6] for d in [-8,-.5,0,.5,4,10,50] for q in [.3,2,12]]
result['noncentral_f']=[dict(df1=d1,df2=d2,**{'lambda':nc},alpha=a,power=fp(d1,d2,nc,a) if nc else a) for d1,d2,nc,a in [(1,1,.5,.05),(100,5,1000,.05),(20,619,96,.05),(3,96,0,.05)]]
Path(__file__).with_suffix('.json').write_text(json.dumps(result,indent=2,allow_nan=False)+'\n',encoding='utf-8')
