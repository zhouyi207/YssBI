# Markov forecast

Connect an ordered categorical, text, identifier, binary or numeric state `series` with at least two nonmissing observations. Numeric states are discrete labels, with exact integer identities retained. States are indexed from zero in first-appearance order; no fixed row or state-count ceiling is imposed.

`ts_horizon=10` is a positive integer. `ts_pseudocount=0` is a finite nonnegative count added to every cell. Zero gives maximum-likelihood transitions; a state with no observed outgoing transitions then makes the transition row unidentified. Set a positive pseudocount to explicitly apply symmetric smoothing.

$$
\widehat P_{ij}=\frac{N_{ij}+a}{\sum_j N_{ij}+Ka},\qquad
\pi_h=e_{s_n}'\widehat P^{\,h}.
$$

$N_{ij}$ counts adjacent transitions, $K$ is the number of observed states, $a$ is the pseudocount and $s_n$ is the last state. The model assumes a homogeneous first-order Markov chain. It neither discretizes continuous values nor estimates hidden states.

`state_labels` preserves original labels; `counts`, `transition_probabilities` and columns of `forecast_probabilities` use that order. `last_state` and `forecast_states` contain zero-based state indices. `forecast_labels` restores the most probable label at each future step, breaking ties by first appearance. These labels are marginal modes of the propagated distributions, not a recursively predicted state path. The first forecast is one step after the final input row. No transition significance test or prediction confidence interval is supplied.
