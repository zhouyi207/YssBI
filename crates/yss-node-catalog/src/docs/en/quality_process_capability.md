# Process capability

Connect numeric **measurements** and set finite **lower_limit**, **upper_limit**
and **target**; the upper specification must exceed the lower. Target affects
Cpm only. At least two finite complete measurements are required.

Without **subgroups**, within sigma is mean consecutive moving range / 1.128,
so input order matters. With an aligned subgroup identifier, within sigma is
the pooled subgroup sample SD: sqrt(sum within-group squared deviations /
sum(n_group−1)). Each subgroup needs at least two observations; unequal sizes
are allowed. This estimator is not c4 bias-corrected.

**result** distinguishes Cp/Cpk using within sigma from Pp/Ppk using overall
sample SD. Cpm uses overall variance plus squared deviation from target.
Negative Cpk/Ppk are retained. Zero denominators produce null, not infinity.
The report also counts observations strictly outside the specifications and
estimates normal-distribution outside PPM under each sigma estimate; these
modeled PPM values are null when the corresponding sigma is zero.

Interpret normal capability indices only with suitable distribution and process
stability evidence. This node does not infer engineering specifications or
declare the process stable/capable automatically. All measurements are used.
