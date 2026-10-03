# DEMATEL influences

Connect a square direct-influence matrix to **criteria**. A(i,j) means row i
influences column j. Use the same criterion order for rows and columns,
nonnegative finite entries, and a zero diagonal. Missing data are rejected.

**influence_normalization** defaults to max_sum: divide by the largest row or
column sum. none uses the original scale. **attenuation** explicitly multiplies
this matrix by a value in (0,1], default 1. The resulting matrix X must have a
spectral radius numerically below 1; divergent or numerically singular systems
raise an error. No unrequested damping is applied.

T = (I − X)⁻¹X includes direct and all indirect influences.
**result** reports normalization, attenuation and spectral radius.
**indices** lists outgoing row sums D, incoming column sums C, prominence D+C,
net cause D−C and normalized prominence weights (null if all influences are zero).
**relations** contains every matrix cell with source/target positions, X and T.

These are relationships encoded by the supplied judgments, not estimated
causal effects from observational data.
