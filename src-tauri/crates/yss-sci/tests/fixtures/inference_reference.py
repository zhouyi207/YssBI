"""Independent references: statsmodels OLS contrasts and multiplicity, SciPy Welch/quantiles."""
import json
from pathlib import Path
import numpy as np
from scipy import stats
import statsmodels.api as sm
from statsmodels.stats.multitest import multipletests

values = [[1., 2., 4., 3., 5.], [3., 4., 8., 5.], [-1., 0., 3., 2., 1., 4.]]
y = np.concatenate(values)
groups = np.repeat(np.arange(3), [len(v) for v in values])
fit = sm.OLS(y, np.eye(3)[groups]).fit()
confidence = .9
cases = []
for pooled in [True, False]:
    for adjustment in ["none", "holm", "bonferroni"]:
        rows = []
        for a in range(3):
            for b in range(a+1, 3):
                if pooled:
                    contrast = np.eye(3)[a]-np.eye(3)[b]
                    result = fit.t_test(contrast)
                    diff, se, df = float(result.effect[0]), float(result.sd[0, 0]), fit.df_resid
                    t, p = float(result.tvalue[0, 0]), float(result.pvalue)
                else:
                    result = stats.ttest_ind(values[a], values[b], equal_var=False)
                    diff, df = np.mean(values[a])-np.mean(values[b]), result.df
                    se = np.sqrt(np.var(values[a], ddof=1)/len(values[a])+np.var(values[b], ddof=1)/len(values[b]))
                    t, p = result.statistic, result.pvalue
                tail = (1-confidence)/(2*(3 if adjustment == "bonferroni" else 1))
                half = stats.t.isf(tail, df)*se
                rows.append(dict(a=a+1, b=b+1, estimate=diff, se=se, df=df, t=t, p=p, lower=diff-half, upper=diff+half))
        p = [row["p"] for row in rows]
        adjusted = p if adjustment == "none" else multipletests(p, method=adjustment)[1]
        for row, adjusted_p in zip(rows, adjusted):
            row["adjusted_p"] = float(adjusted_p)
        cases.append(dict(pooled=pooled, adjustment=adjustment, rows=rows))
result = dict(y=y.tolist(), groups=groups.tolist(), confidence=confidence, cases=cases,
              normal_q=stats.norm.isf(.025), t_q=stats.t.isf(.025, 12.5))
Path(__file__).with_suffix(".json").write_text(json.dumps(result, indent=2, allow_nan=False)+"\n", encoding="utf8")
