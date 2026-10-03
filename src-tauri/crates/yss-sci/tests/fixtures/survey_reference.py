"""statsmodels coefficient fits; NumPy implementation of survey's stratum/PSU variance formula."""
import json
from pathlib import Path
import numpy as np
import statsmodels.api as sm
from scipy.stats import t

i=np.arange(640)
x=(i%16)/5-1.5
z=(i*13%17)/8-1
w=1+(i%7)*.2
h=i//160
g=(i//8)%20
y=4+.5*x-.3*z+.4*((i//8)%7)-1.2+.03*((i*11%19)-9)
binary=((i*37%101)<(45+3*x-2*z)).astype(float)
counts=(i*7%6)+(x>0)

def covariance(scores,strata,clusters):
    result=np.zeros((scores.shape[1],scores.shape[1]))
    for stratum in np.unique(strata):
        ids=np.unique(clusters[strata==stratum])
        totals=np.array([scores[(strata==stratum)&(clusters==j)].sum(axis=0) for j in ids])
        centered=totals-totals.mean(axis=0)
        result+=len(ids)/(len(ids)-1)*centered.T@centered
    return result

result={'weights':dict(sum=w.sum(),kish=w.sum()**2/(w@w))}
for key,hh,gg in [('iid',np.zeros(640),i),('stratified',h,i),('clustered',np.zeros(640),g),('nested',h,g)]:
    value=np.sum(y*w)/w.sum()
    v=covariance((w*(y-value)/w.sum())[:,None],hh,gg)[0,0]
    df=sum(len(np.unique(gg[hh==a]))-1 for a in np.unique(hh))
    result[key]=dict(mean=value,se=np.sqrt(v),df=df,ci=(value+np.array([-1,1])*t.ppf(.975,df)*np.sqrt(v)).tolist())
value=np.sum(binary*w)/w.sum()
result['proportion']=dict(mean=value,se=np.sqrt(covariance((w*(binary-value)/w.sum())[:,None],h,g)[0,0]))
X=sm.add_constant(np.column_stack([x,z]))
wn=w/w.mean()
for key,response,family in [('gaussian',y,sm.families.Gaussian()),('binomial',binary,sm.families.Binomial()),('poisson',counts,sm.families.Poisson())]:
    fit=sm.GLM(response,X,family=family,freq_weights=wn).fit(tol=1e-12)
    mu=fit.fittedvalues
    var=family.variance(mu)
    bread=np.linalg.inv(X.T@((wn*var)[:,None]*X))
    score=X*(wn*(response-mu))[:,None]
    cov=bread@covariance(score,h,g)@bread
    se=np.sqrt(np.diag(cov))
    df=80-4-3+1
    result[key]=dict(beta=fit.params.tolist(),covariance=cov.tolist(),se=se.tolist(),p=(2*t.sf(abs(fit.params/se),df)).tolist(),last_fitted=mu[-1],df=df)
Path(__file__).with_suffix('.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
