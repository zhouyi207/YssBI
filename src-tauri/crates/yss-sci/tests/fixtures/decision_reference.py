"""Independent NumPy/SciPy/statsmodels and PyMCDM 1.4 references.

PyMCDM supplies CRITIC and TOPSIS. SciPy entropy handles zero probabilities;
PyMCDM 1.4's entropy implementation assigns zero entropy to any column with zeros.
Install pymcdm and tabulate only for regeneration, not for Rust/runtime execution.
"""
import json
from pathlib import Path
import numpy as np
from scipy.stats import entropy, rankdata
import statsmodels.api as sm
from pymcdm import weights as mcdm_weights, normalizations
from pymcdm.methods import TOPSIS, VIKOR

x = np.array([[3,15,2],[9,12,5],[4,18,9],[7,8,6],[8,16,4],[2,10,7],[6,14,3],[3,15,2]], dtype=float)
types = np.array([1,-1,1])
z = (x-x.min(axis=0))/np.ptp(x,axis=0)
benefits=z.copy()
benefits[:,types==-1]=1-benefits[:,types==-1]
e = entropy(benefits,axis=0)/np.log(len(x))
cv = np.std(x,axis=0,ddof=1)/np.mean(x,axis=0)
r = np.array([np.sqrt(sm.OLS(x[:,j],sm.add_constant(np.delete(x,j,axis=1))).fit().rsquared) for j in range(x.shape[1])])
w = {"equal":np.ones(3)/3,"explicit":np.array([.5,.3,.2]),"entropy":(1-e)/(1-e).sum(),
     "critic":mcdm_weights.critic_weights(benefits),"information":cv/cv.sum(),"independence":(1/r)/(1/r).sum()}
cases=[]
for method, weighting, normalization in [
    ("weights","entropy","minmax"),("entropy_weight","entropy","minmax"),("critic","critic","minmax"),
    ("information_weight","information","none"),("independence_weight","independence","none"),
    ("composite_index","equal","minmax"),("topsis","explicit","vector"),("grey_relational","explicit","minmax"),
    ("wrsr","explicit","none"),("efficacy_coefficient","explicit","minmax"),("entropy_topsis","entropy","vector")]:
    if method in ["topsis","entropy_topsis"]:
        scores=TOPSIS(normalization_function=normalizations.vector_normalization)(x,w[weighting],types)
    elif method=="grey_relational":
        delta=1-benefits
        scores=(.5/(delta+.5))@w[weighting]
    elif method=="wrsr":
        ranks=rankdata(x,axis=0,method="average")
        ranks[:,types==-1]=len(x)+1-ranks[:,types==-1]
        scores=(ranks/len(x))@w[weighting]
    elif method=="efficacy_coefficient":
        scores=(60+40*benefits)@w[weighting]
    elif normalization=="none":
        scores=(x*types)@w[weighting]
    else:
        scores=benefits@w[weighting]
    cases.append(dict(method=method,weighting=weighting,normalization=normalization,scores=scores.tolist(),ranks=rankdata(-scores,method="average").tolist()))
    if method == "wrsr":
        repeated = np.tile(x, (80, 1))
        ranks = rankdata(repeated, axis=0, method="average")
        ranks[:, types == -1] = len(repeated)+1-ranks[:, types == -1]
        cases[-1]["scores_640_tail"] = ((ranks/len(repeated))@w[weighting])[-8:].tolist()
    else:
        cases[-1]["scores_640_tail"] = scores.tolist()
result=dict(columns=x.T.tolist(),costs=[False,True,False],weights={name:value.tolist() for name,value in w.items()},
            entropy=e.tolist(),cv=cv.tolist(),multiple_r=r.tolist(),cases=cases)
result["vikor"] = [dict(v=v, scores=VIKOR(v=v)(x,w["explicit"],types).tolist()) for v in [0.,0.5,1.]]
Path(__file__).with_suffix(".json").write_text(json.dumps(result,indent=2,allow_nan=False)+"\n",encoding="utf8")
