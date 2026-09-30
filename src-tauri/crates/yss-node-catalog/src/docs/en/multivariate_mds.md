# Multidimensional scaling (MDS)

This node uses classical metric MDS. **MDS input** defaults to `observations`: provide 1–16 numeric **Variable / distance column** series with observations in rows, using Euclidean distances. **Standardize variables** defaults to disabled and appears only in this mode; enabling it uses sample standard deviations and rejects constant variables.

With `dissimilarity_matrix`, each input is one column of a nonnegative symmetric square distance matrix with zero diagonal, at most 512 rows/columns. Symmetry and zero diagonal allow floating error of $10^{-12}$ relative to maximum distance. No additive distance correction is applied. Missing values and distances without positive variation are rejected. Columns must align and relational inputs must share a row domain.

The observation count is $n\le512$. **Retained dimensions** defaults to 2 with $1\le k\le\min(n-1,16)$. For distances $D$ and centering $J=I-\mathbf1\mathbf1^T/n$:

$$B=-\tfrac12J D^{\circ2}J=V\Lambda V^T,\qquad T_k=V_k\operatorname{diag}(\sqrt{\max(\lambda_j,0)}).$$

Leading positive eigenvalues define axes; dimensions beyond positive rank have zero coordinates. Non-Euclidean distances may produce negative eigenvalues, whose count and absolute inertia are reported explicitly rather than treated as ordinary variance.

**Result** contains retained eigenvalues, positive rank, positive/negative inertia, two fit proportions using positive and absolute inertia, and distance stress

$$\mathrm{stress}=\sqrt{\frac{\sum_{i<j}(d_{ij}-\widehat d_{ij})^2}{\sum_{i<j}d_{ij}^2}}.$$

$\widehat d_{ij}$ is embedded Euclidean distance. Smaller stress means better distance recovery in the selected dimensions; it is not nonmetric rank stress. No hypothesis test or p-value is provided.

**Coordinates** is an independent computed table with `axis1` through `axisK` in observation/distance-row order, supporting column selection and plotting. Rotation, reflection and sign changes preserve embedded distances.

Reference: [R cmdscale](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/cmdscale.html).
