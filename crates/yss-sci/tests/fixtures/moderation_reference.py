"""statsmodels OLS/t contrasts and NumPy quadratic roots, independent of SCI."""
import json
from pathlib import Path
import numpy as np
import statsmodels.api as sm
from scipy.stats import t
i=np.arange(640)
x=i%8-3.5
w=10+(i//8)%8
z=20+(i//64)%10
c=(i*19%23)/11-1
out={}
for advanced in [False,True]:
    raw=[x,w,z,c] if advanced else [x,w,c]
    centered=[a-a.mean() for a in raw]
    a,b=centered[:2]
    cov=centered[-1]
    noise=.05*((i*7)%17-8)
    y=1+.3*a+.2*b+.5*a*b+.3*c+noise
    basis=[a,b,a*b]
    if advanced:
        d=centered[2]
        y=y+.4*d-.12*a*d+.06*b*d+.08*a*b*d
        basis=[a,b,d,a*b,a*d,b*d,a*b*d]
    fit=sm.OLS(y,sm.add_constant(np.column_stack(basis+[cov]))).fit()
    probes=[];regions=[]
    for zz in [-z.std(ddof=1),0,z.std(ddof=1)] if advanced else [0]:
        aa=np.zeros(len(fit.params));bb=aa.copy();aa[1]=1
        if advanced: aa[5]=zz;bb[4]=1;bb[7]=zz
        else: bb[3]=1
        for ww in [-w.std(ddof=1),0,w.std(ddof=1)]:
            test=fit.t_test(aa+ww*bb)
            probes.append(dict(w=float(ww+w.mean()),z=float(zz+z.mean()) if advanced else None,
                estimate=float(test.effect[0]),se=float(test.sd[0,0]),p=float(test.pvalue)))
        va,vab,vb=aa@fit.cov_params()@aa,aa@fit.cov_params()@bb,bb@fit.cov_params()@bb
        acoef,bcoef=aa@fit.params,bb@fit.params
        crit=t.ppf(.975,fit.df_resid)**2
        roots=np.roots([bcoef*bcoef-crit*vb,2*(acoef*bcoef-crit*vab),acoef*acoef-crit*va])
        roots=sorted(float(r.real+w.mean()) for r in roots if abs(r.imag)<1e-10 and w.min()<r.real+w.mean()<w.max())
        regions.append(roots)
    out['advanced' if advanced else 'simple']=dict(coefficients=fit.params.tolist(),slopes=probes,boundaries=regions,last_fitted=fit.fittedvalues[-1])
Path(__file__).with_suffix('.json').write_text(json.dumps(out,indent=2)+'\n')
