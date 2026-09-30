# Coefficient Plot

Model accepts the OLS, WLS or GLS model output from Linear Fit without refitting. Confidence level defaults to 0.95 and must be strictly between 0 and 1. Intercepts are hidden by default.

Points show coefficient estimates and horizontal intervals show $\hat\beta_j\pm t_{1-\alpha/2,df_r}\,SE(\hat\beta_j)$, where $1-\alpha$ is confidence coverage and $df_r$ is residual degrees of freedom. Standard errors use the fitted model's covariance method. The zero line helps identify intervals containing 0.

Open the result output in a workbench result panel or a separate Plot window.
