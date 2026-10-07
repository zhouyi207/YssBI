# Quality control chart (I / MR)

Connect a numeric **measurements** series in process/time order, with at least
two complete observations. Missing and nonfinite values are rejected. The node
does not sort observations.

Choose **chart_kind**: individuals (I), or moving range (MR). MR is the absolute
difference of consecutive measurements. The within-process sigma estimate is
mean(MR)/1.128. I limits are the mean ± 3 sigma; MR limits are 0 and
3.267 × mean(MR), with mean(MR) as the center.

**result** draws all chart points and three reference lines. **summary** reports
limits, sigma and the count strictly outside limits. **observations** preserves
every plotted value and its outside indicator (0/1); the first MR point belongs
to observation 2. No display subsampling can hide a signal.

These are Phase-I limits estimated from the supplied observations and the
single-point Shewhart rule. There is no automatic removal/refitting of signals,
run-rule assessment or subgroup/attribute chart in this node. Constant data
produce coincident limits. Control limits describe process variation; they are
not engineering specification limits or proof of stability.
