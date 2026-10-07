"""Independent matrix references: NumPy, PyMCDM 1.4.0; not runtime dependencies."""
import json
from pathlib import Path
import numpy as np
from pymcdm.weights.subjective import AHP

a = np.array([[1, 1/3, 1/2, 1/2], [3, 1, 2, 2],
              [2, 1/2, 1, 2], [2, 1/2, 1/2, 1]])
ahp = AHP(matrix=a)
w = ahp()
eigen = np.linalg.eigvals(a).real.max()
f = np.array([[.5, .7, .6, .7], [.3, .5, .6, .6],
              [.4, .4, .5, .6], [.3, .4, .4, .5]])
fw = (f.sum(axis=1) + len(f)/2 - 1) / (len(f)*(len(f)-1))
compatibility = np.abs(f - fw[:, None]/(fw[:, None]+fw[None, :])).mean()
d = np.array([[0., 2, 1], [1, 0, 1], [1, 1, 0]])
x = d/max(d.sum(axis=0).max(), d.sum(axis=1).max())
t = np.linalg.solve(np.eye(len(x))-x, x)
out = {
    "ahp": {"columns": a.T.tolist(), "weights": w.tolist(),
            "lambda": eigen, "cr": ahp.get_cr()},
    "fahp": {"columns": f.T.tolist(), "weights": fw.tolist(),
             "compatibility": compatibility},
    "dematel": {"columns": d.T.tolist(), "total": t.tolist(),
                "outgoing": t.sum(axis=1).tolist(), "incoming": t.sum(axis=0).tolist()},
}
Path(__file__).with_suffix(".json").write_text(json.dumps(out, indent=2)+"\n", encoding="utf-8")
