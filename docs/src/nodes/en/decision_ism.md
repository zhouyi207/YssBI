# Interpretive structural modeling

Connect a square 0/1 adjacency matrix to **criteria**. Row i points to column j
when A(i,j) = 1. Row and column positions must refer to the same factors.
The diagonal may be 0 or 1; the reflexive reachability relation includes every
factor itself. Missing and nonfinite values are rejected.

The node computes full transitive closure, then collapses mutually reachable
factors into strongly connected components. Level 1 contains sink components
(final outcomes); removing them successively yields higher-numbered cause levels.
Cycles remain together and no factors are silently omitted.

**result** reports levels and strongly connected components.
**indices** contains factor position, level, driving power and dependence,
including reflexive reachability. **relations** contains the full 0/1 reachability
matrix as source/target/value rows. Independent components may share a level.
