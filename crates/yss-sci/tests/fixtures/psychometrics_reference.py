"""Independent NumPy/SciPy reference for complete-case scale statistics."""
import json
from pathlib import Path
import numpy as np
from scipy import stats

i = np.arange(640)
latent = (i*13) % 37
x = np.column_stack([
    1 + .07*latent + .13*((i*17) % 11),
    2 + .11*latent + .17*((i*19) % 13),
    .4 + .09*latent + .05*((i*23) % 17),
    3 + .15*latent + .11*((i*29) % 19),
])
def alpha(data):
    cov = np.cov(data, rowvar=False)
    p = cov.shape[0]
    return p/(p-1)*(1-np.trace(cov)/cov.sum())
r = np.corrcoef(x, rowvar=False)
inv = np.linalg.inv(r)
partial = inv/np.sqrt(np.outer(inv.diagonal(), inv.diagonal()))
r2, p2 = r*r, partial*partial
np.fill_diagonal(r2, 0)
np.fill_diagonal(p2, 0)
n, p = x.shape
chi = -(n-1-(2*p+5)/6)*np.linalg.slogdet(r)[1]
totals = x.sum(axis=1)
lo, hi = np.quantile(totals, [.27, .73], method="linear")
low, high = x[totals <= lo], x[totals >= hi]
tt = stats.ttest_ind(high, low, axis=0, equal_var=False)
out = {
    "raw_alpha": alpha(x), "standardized_alpha": alpha((x-x.mean(0))/x.std(0,ddof=1)),
    "means": x.mean(0).tolist(), "sd": x.std(0,ddof=1).tolist(),
    "alpha_deleted": [alpha(np.delete(x,j,axis=1)) for j in range(p)],
    "corrected_correlations": [np.corrcoef(x[:,j],np.delete(x,j,axis=1).sum(1))[0,1] for j in range(p)],
    "kmo": r2.sum()/(r2.sum()+p2.sum()),
    "msa": (r2.sum(0)/(r2.sum(0)+p2.sum(0))).tolist(),
    "bartlett": chi, "bartlett_df": p*(p-1)//2, "bartlett_p": stats.chi2.sf(chi,p*(p-1)//2),
    "low_cutoff": lo, "high_cutoff": hi, "low_count": len(low), "high_count": len(high),
    "low_mean":low.mean(0).tolist(), "high_mean":high.mean(0).tolist(),
    "t":tt.statistic.tolist(), "df":tt.df.tolist(), "p":tt.pvalue.tolist(),
}
Path(__file__).with_suffix(".json").write_text(json.dumps(out,indent=2)+"\n",encoding="utf-8")
