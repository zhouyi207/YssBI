# Redundancy analysis (RDA)

Provide one or more numeric **Y₁, Y₂, …** and one or more numeric **Constraint** series in aligned rows, paired by current position, including mixed database and in-memory inputs. Missing data are rejected. With $p$ responses, $r$ constraints and $n$ observations, constraints must be nonconstant and full rank, and $n>r+1$.

**Retained dimensions** defaults to 1 and cannot exceed $\min(p,r)$. **Standardize variables** defaults to disabled: responses are centered in original units; enabling it divides responses by sample standard deviations. Constraints are always centered/scaled without changing their spanned space. Positive total response variation is required.

RDA fits multivariate least squares and applies PCA to fitted responses:

$$\widehat Y=X(X^TX)^{-1}X^TY,\qquad E=Y-\widehat Y,\qquad T=\widehat Y V_k.$$

$V_k$ contains leading fitted-response covariance eigenvectors. Outputs use direct fitted-response scores rather than alternative ordination plotting scalings.

**Permutation draws** defaults to 199 (0–9999); 0 disables inference. **Random seed** defaults to 42. The overall null is no association between constraints and multivariate responses, versus an association. The pseudo-F and plus-one p-value are

$$F=\frac{SS_{\mathrm{fit}}/r}{SS_{\mathrm{error}}/(n-r-1)},\qquad p=\frac{1+\#\{F_b\ge F\}}{B+1}.$$

$B$ is the draw count and $SS$ sums over all responses. Whole response rows are permuted, preserving within-row multivariate structure. Inference requires exchangeable observations under the null and does not handle restricted block/serial permutations. Perfect fits have null F/permutation p-values.

**Result** gives total/constrained/residual inertia (sums of squares divided by $n-1$), both eigenvalue sets, response weights, $R^2$ and adjusted $1-(1-R^2)(n-1)/(n-r-1)$, F, degrees of freedom and permutation details. Adjusted $R^2$ may be negative.

**Scores** is an independent computed fitted-response table with `axis1` through `axisK` in input row order, usable for selection, plotting or further analysis. Editing does not scan observations or run inference.

Reference: [vegan RDA](https://vegandevs.github.io/vegan/reference/cca.html).
