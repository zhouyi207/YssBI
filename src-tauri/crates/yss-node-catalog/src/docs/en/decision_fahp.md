# Fuzzy AHP weights

Connect a square fuzzy complementary judgment matrix to **criteria**, one
column per criterion. Rows use the same criterion order as columns.
Entries are in [0,1], diagonal entries are 0.5, and A(i,j) + A(j,i) = 1
(absolute tolerance 1e−8). Missing/nonfinite values are rejected.

This implements the complementary-matrix FAHP convention:
w(i) = [sum_j A(i,j) + n/2 − 1]/[n(n − 1)], followed by normalization.
One criterion receives weight 1.
**result** reports weights and the compatibility index
mean_ij |A(i,j) − w(i)/(w(i)+w(j))|, with an acceptance threshold of 0.1.
**weights** exposes the reusable normalized vector.

This convention uses scalar complementary preferences; it is not triangular
fuzzy-number extent analysis.
Reference: [FAHP formulas](https://pubimage.spssau.com/algo/fahp.pdf).
