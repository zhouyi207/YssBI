# Correspondence analysis

Provide **Count column** series from a nonnegative two-way frequency/weight table with at least two rows and two aligned columns. Rows and columns denote categories rather than individual observations. Negative/missing entries and zero-mass rows or columns are rejected. **Retained dimensions** defaults to 1 and cannot exceed $\min(r-1,c-1)$.

With total weight $N$, relative frequencies $P$, row/column masses $a,b$ and their diagonal matrices $D_a,D_b$:

$$S=D_a^{-1/2}(P-ab^T)D_b^{-1/2}=U\Sigma V^T,\quad F=D_a^{-1/2}U\Sigma,\quad G=D_b^{-1/2}V\Sigma.$$

$F,G$ are principal row/column coordinates. Axis inertia is $\lambda_j=\sigma_j^2$, total inertia $I=\sum_j\lambda_j$, and the descriptive Pearson quantity is $NI$.

**Result** reports masses, eigenvalues, inertia/retained proportions and `chi_square`. Noninteger weights are supported and no p-value is computed: arbitrary-weight `chi_square` is not automatically a frequency-table independence test. An independent table has zero inertia/coordinates and null undefined inertia proportions.

**Row coordinates** and **Column coordinates** are independent numeric tables with `axis1` through `axisK`, ordered by input row categories or input column order. They support column selection and plotting. Different category domains are not aligned merely by equal length. Euclidean distances between a row category and a column category generally do not represent direct category distances.

Reference: [R MASS corresp](https://stat.ethz.ch/R-manual/R-devel/library/MASS/html/corresp.html).
