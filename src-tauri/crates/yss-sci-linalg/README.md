# yss-sci-linalg

Owns opaque `Mat`, `Col`, row and borrowed-view types, matrix arithmetic, checked
factorizations, stable numerical errors and the rank/conditioning convention.
faer is an implementation dependency of this crate alone. Public APIs and macros
expose project types, with no native-type re-export, conversion escape hatch or
`Deref` to faer. The faer version remains in shared workspace dependencies.

```rust
use yss_sci_linalg::{MatrixExt, Solve, col, mat};

let a = mat![[4.0, 1.0], [1.0, 3.0]];
let b = col![6.0, 7.0];
let factor = a.checked_cholesky()?;
let x = factor.solve(&b);
let reconstructed = &a * &x;
```

## Contract

- Matrix multiplication, slicing and triangular in-place solves delegate to faer
  internally. Statistical formulas and elementwise operations stay in SCI.
- `MatrixExt::checked_cholesky` and `checked_lu` preserve project error semantics.
  Factors are reusable; `Solve` accepts owned or borrowed matrix/vector RHS values.
  Cholesky reads the lower triangle. LU rejects an exactly zero pivot without
  applying an additional rank tolerance.
- `Svd::factor` returns full left/right vectors and descending singular values.
  Views borrow the decomposition without copying its factors. `matrix_rank`
  requests singular values only, avoiding square left-vector allocation for tall
  matrices. Its tolerance is `max(rows, cols) * f64::EPSILON * largest_value`;
  empty matrices return `(0, 1)` and zero matrices `(0, infinity)`. The relative
  tolerance factor is evaluated first to avoid intermediate overflow. Nonfinite
  inputs or singular values return `DecompositionFailed`, never a zero rank.
- Symmetric eigenvalues are ascending and use the lower triangle. General
  eigenvalues are complex and unordered; vector column `i` matches eigenvalue `i`.
  Normalization and phase are not portable promises. Callers sort when required.
- RHS dimensions are programmer preconditions; nonsquare matrices and numerical
  decomposition failures use `LinalgError`. Callers must distinguish a failed
  decomposition from a successfully computed zero rank.
- Borrowed wrappers preserve logical indices and may be strided or reversed.
  Owned matrices are column major and may have padding. Serialized row order must
  be assembled explicitly; native storage must not be treated as a flat row-major
  array. There is no second array representation or raw-pointer conversion layer.

The crate owns no statistical models, project state, reports or transport.
Changes must preserve numerical residual, view, rank and SCI golden tests.
Numerical equivalence uses tolerances, not bitwise equality. macOS, Windows and
Linux remain target platforms; tests on one platform do not establish all three.
