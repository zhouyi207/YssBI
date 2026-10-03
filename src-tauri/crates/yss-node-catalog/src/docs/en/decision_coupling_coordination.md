# Coupling coordination

Connect at least two aligned Numeric **criteria**, each representing a subsystem index
already on [0,1]. Values must be finite and nonmissing. No hidden rescaling or positive
offset is applied. There is no fixed observation-count limit.

Optional **criterion_weights** supplies a separate nonnegative weight vector in subsystem
order. Weights are normalized; omitted weights are equal. Weights determine T, while the
coupling C compares the subsystem levels symmetrically.

For $m$ subsystem indices:

$$
C_i=\frac{(\prod_j U_{ij})^{1/m}}{\sum_j U_{ij}/m},\qquad
T_i=\sum_j w_jU_{ij},\qquad D_i=\sqrt{C_iT_i}.
$$

**scores** returns every observation with coupling C, coordination index T and coordination
degree D. Geometric means use logarithms to avoid product underflow. With a zero subsystem
and a positive overall mean, C is zero. If all subsystems are zero, C is undefined (null),
T is zero and D takes its continuous limit zero. **result** reports the number of rows with
undefined C; **weights** returns normalized weights.

C measures balance between levels; uniformly low indices can have high C. D also includes
development level. No empirical quality grades or causal interpretation are imposed.
