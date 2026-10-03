"""Independent observed-path reference: statsmodels OLS and NumPy matrix inverse."""
import json
from pathlib import Path
import numpy as np
import statsmodels.api as sm

i = np.arange(640)
x = i % 8 - 3.5
w = (i // 8) % 8 - 3.5
c = (i * 19 % 23) / 11 - 1
result = {}
for stage in ("none", "first", "second"):
    m = 2 + .6*x + .2*w + .3*c + .3*((i*7 % 17)-8)
    if stage == "first":
        m = m + .15*x*w
    mc = m - np.mean(m)
    y = 1 + .25*x + .7*mc + .2*w + .3*c + .2*((i*11 % 19)-9)
    if stage == "second":
        y = y + .08*mc*w
    columns = [x, m] + ([w+10] if stage != "none" else []) + [c]
    centered = [a-np.mean(a) for a in columns]
    ma = [centered[0]]
    ya = [centered[0], centered[1]]
    if stage != "none":
        ma.append(centered[2]); ya.append(centered[2])
    if stage == "first": ma.append(centered[0]*centered[2])
    if stage == "second": ya.append(centered[1]*centered[2])
    ma.append(centered[-1]); ya.append(centered[-1])
    mf = sm.OLS(m, sm.add_constant(np.column_stack(ma))).fit()
    yf = sm.OLS(y, sm.add_constant(np.column_stack(ya))).fit()
    a,b = mf.params,yf.params
    probes = np.array([-1,0,1])*np.std(w,ddof=1) if stage != "none" else [0]
    indirect = [(a[1]+(a[3]*v if stage=="first" else 0))*(b[2]+(b[4]*v if stage=="second" else 0)) for v in probes]
    result[stage] = dict(mediator=a.tolist(),outcome=b.tolist(),mediator_se=mf.bse.tolist(),outcome_se=yf.bse.tolist(),indirect=indirect,total=[b[1]+v for v in indirect],index=None if stage=="none" else a[3]*b[2] if stage=="first" else a[1]*b[4],last_m=mf.fittedvalues[-1],last_y=yf.fittedvalues[-1])
    if stage=="none":
        z=-.4*x+.2*m+.5*y+.15*((i*13 % 29)-14)
        data=[x,m,y,z,c]
        equations=[(3,[0,1,2]),(2,[0,1,4]),(1,[0,4])]
        direct=np.zeros((5,5))
        fits=[]
        for target,predictors in equations:
            fit=sm.OLS(data[target],sm.add_constant(np.column_stack([data[j] for j in predictors]))).fit()
            direct[target,predictors]=fit.params[1:]
            fits.append(fit.params.tolist())
        total=np.linalg.inv(np.eye(5)-direct)-np.eye(5)
        sd=np.std(data,axis=1,ddof=1)
        result['recursive']=dict(fits=fits,direct=direct.tolist(),total=total.tolist(),standardized=(total*sd[None,:]/sd[:,None]).tolist())
Path(__file__).with_suffix('.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
