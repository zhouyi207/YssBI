"""Independent process/crossed-study reference: NumPy, SciPy, statsmodels."""
import json
from pathlib import Path
import numpy as np
import pandas as pd
from scipy import stats
from statsmodels.formula.api import ols
from statsmodels.stats.anova import anova_lm

x=np.array([49.6,47.6,49.9,51.3,47.8,51.2,52.6,52.4,53.6,52.1])
mean,overall,mr=x.mean(),x.std(ddof=1),np.abs(np.diff(x)).mean()
within=mr/1.128
low,high,target=45.,56.,50.5
cap={"mean":mean,"overall":overall,"within":within,"mr":mr,
     "cp":(high-low)/(6*within),"cpk":min(high-mean,mean-low)/(3*within),
     "pp":(high-low)/(6*overall),"ppk":min(high-mean,mean-low)/(3*overall),
     "cpm":(high-low)/(6*np.hypot(overall,mean-target)),
     "within_ppm":1e6*(stats.norm.cdf((low-mean)/within)+stats.norm.sf((high-mean)/within)),
     "overall_ppm":1e6*(stats.norm.cdf((low-mean)/overall)+stats.norm.sf((high-mean)/overall)),
     "pooled":np.sqrt(((x[:5]-x[:5].mean())**2).sum()/8+((x[5:]-x[5:].mean())**2).sum()/8)}
i=np.arange(720)
part,operator=i//72,(i//24)%3
y=10+.7*part+.25*operator+.12*part*operator+((i*17)%29-14)*.03
data=pd.DataFrame({"y":y,"part":part,"operator":operator})
table=anova_lm(ols("y ~ C(part, Sum)*C(operator, Sum)",data=data).fit(),typ=2)
ss=table["sum_sq"].to_numpy()
df=table["df"].to_numpy()
ms=ss/df
def report(interaction):
    mse=ms[3] if interaction else (ss[2]+ss[3])/(df[2]+df[3])
    denominator=ms[2] if interaction else mse
    vp=max((ms[0]-denominator)/(3*24),0)
    vo=max((ms[1]-denominator)/(10*24),0)
    vi=max((ms[2]-mse)/24,0) if interaction else 0
    components=np.array([mse,vo,vi,vo+vi,mse+vo+vi,vp,mse+vo+vi+vp])
    dfs=[df[0],df[1],df[2],df[3]] if interaction else [df[0],df[1],df[2]+df[3]]
    sums=[*ss] if interaction else [ss[0],ss[1],ss[2]+ss[3]]
    denom_df=df[2] if interaction else df[2]+df[3]
    f=[ms[0]/denominator,ms[1]/denominator]+([ms[2]/ms[3]] if interaction else [])
    p=[stats.f.sf(f[0],df[0],denom_df),stats.f.sf(f[1],df[1],denom_df)]+([stats.f.sf(f[2],df[2],df[3])] if interaction else [])
    return {"ss":sums,"df":dfs,"components":components.tolist(),"f":f,"p":p}
out={"capability":cap,"gage_interaction":report(True),"gage_pooled":report(False)}
Path(__file__).with_suffix(".json").write_text(json.dumps(out,indent=2)+"\n",encoding="utf-8")
