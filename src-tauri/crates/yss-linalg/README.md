# yss-linalg

Backend-independent dense `f64` linear algebra for SCI algorithms and runtime
diagnostics. `ndarray` owns the public data vocabulary; the private `backend`
module is the workspace's only consumer of `faer`.

The faer version and features are declared in this crate's `Cargo.toml`, outside
the shared workspace dependencies. Other crates depend on the interfaces here.

Import `MatMul`, `MatrixExt` and `Solve` to multiply matrices, factor a matrix and
solve with the resulting factors:

```rust
use ndarray::array;
use yss_linalg::{MatMul, MatrixExt, Solve};

let a = array![[4.0, 1.0], [1.0, 3.0]];
let b = array![6.0, 7.0];
let factor = a.cholesky()?;
let x = factor.solve(&b);
let reconstructed = a.matmul(&x);
```

## Contract

- `MatMul` supports matrix/matrix and matrix/vector products. Existing ndarray
  elementwise operations and statistical formulas remain with their callers.
- `MatrixExt` provides Cholesky, partial-pivot LU, full SVD and singular values.
  Cholesky and LU own reusable factors; `Solve` accepts one or multiple RHS.
- Cholesky and `SymmetricEigen::factor` read the lower triangle of a square
  matrix. `SolveLowerTriangular` overwrites an exclusive RHS view.
- SVD returns full left/right vectors and descending singular values. Its views
  borrow the decomposition without copying the factors. `matrix_rank` computes only
  singular values, so tall input matrices do not allocate a square left-vector matrix. It uses
  `max(rows, cols) * f64::EPSILON * largest_singular_value`; empty matrices return
  `(0, 1)`, and zero matrices return `(0, infinity)`. Failed decompositions return
  `LinalgError`; SCI callers retain their existing rank-failure fallback.
- Symmetric eigenvalues are ascending. General eigenvalues are complex and
  unordered; vector column `i` corresponds to eigenvalue `i`. Normalization and
  phase are not portable promises. Callers that require ordering sort explicitly.
- Dimensions for multiplication and RHS solves are asserted programmer
  preconditions. Nonsquare decompositions and numerical failures use typed
  errors. LU rejects an exactly zero pivot; it does not apply a rank tolerance.
- Inputs may be transposed, sliced, reversed or shared broadcast views. Mutable
  inputs must satisfy ndarray's exclusive-view invariants. Backend layout and
  all raw-pointer conversion stay private. Owned matrix outputs are column major.

The crate has no statistical models, project state, report types, transport,
backend registry or runtime selection. A replacement backend must preserve these
contracts and pass the numerical residual, stride and SCI golden tests; numerical
results are compared with tolerances, not bitwise equality.
