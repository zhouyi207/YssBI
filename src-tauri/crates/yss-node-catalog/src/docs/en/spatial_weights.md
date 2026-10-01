# Spatial weights

Build a reusable weights design from unit identifiers and planar coordinates, then connect it to Moran, spatial regression or spatial panel nodes.

## Inputs and parameters

`units` contains unique, nonmissing identifiers; `x` and `y` are finite numeric coordinates sharing the same row domain. At least two units are required. Missing values are rejected; text and numeric identifiers are distinct and integers retain exact precision. For a panel, supply a separate one-row-per-unit coordinate table to this node.

- `spatial_weight_rule`: `knn` (default), `distance_band` or `inverse_distance`.
- `spatial_neighbors`: 4 by default; KNN requires $1\le k<n$. Equal distances are resolved by input unit order. Neighbor links can be directed.
- `spatial_radius`: positive, default 1. Band and inverse-distance rules include neighbors exactly at the threshold.
- `spatial_power`: positive, default 1. Inverse distance uses $d_{ij}^{-p}$ and rejects coincident coordinates inside the threshold.
- `spatial_symmetrize`: false by default. Take the union of links, $w_{ij}\leftarrow\max(w_{ij},w_{ji})$.
- `spatial_row_standardize`: true by default. After symmetrization, divide each nonzero row by its sum. Numerical symmetry need not survive row standardization.

## Computation and output

Euclidean distance is $d_{ij}=\sqrt{(x_i-x_j)^2+(y_i-y_j)^2}$. KNN and distance bands give binary links, while inverse distance gives decaying weights. The diagonal is always zero. At least one link is required; islands retain zero rows.

`result` contains `units`, `matrix`, `options` and `islands`. `matrix[i][j]` is the contribution of unit $j$ to the spatial lag at unit $i$. `units` preserves input order; `islands` contains zero-based unit indices. Downstream nodes explicitly align identifiers, so sorting observations does not change spatial identity. Cross sections must cover the same unit set; every panel period must cover all units.

This is a design object, not a significance test; no p-values are produced. Distances use coordinate units. Longitude/latitude is not automatically treated as spherical: project coordinates before use. Polygon topology and Queen/Rook contiguity are not implemented here. Dense matrices and structured output are checked against the execution memory budget.

[Neighbors and identifiers](https://pysal.org/libpysal/stable/generated/libpysal.weights.KNN.html) · [Weight transformations](https://pysal.org/libpysal/stable/generated/libpysal.weights.W.html)
