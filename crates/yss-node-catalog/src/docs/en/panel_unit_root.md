# Panel unit root (Fisher–ADF)

Connect aligned finite numeric **series**, **entity** IDs and integer period indices **time**, without missing values. Rows are sorted by entity/time. Every entity must have unique consecutive periods. Start/end periods and lengths may differ across entities; at least two entities are required.

**Parameters:** lags defaults to 1, a nonnegative integer, specifying the number of lagged differences in every individual ADF regression. regression defaults to constant; alternatives are none and trend (constant plus linear trend). Lag selection is fixed, not automatic.

For each entity estimate

$$
\Delta y_{it}=a_i+b_it+\gamma_i y_{i,t-1}
+\sum_{j=1}^{p}\delta_{ij}\Delta y_{i,t-j}+u_{it}.
$$

regression selects $a_i,b_i$; $p$ is lags. Individual statistics are $\hat\gamma_i/SE(\hat\gamma_i)$, calibrated using the MacKinnon unit-root distribution matching the deterministic terms, not an ordinary t tail.

The panel null is that all entities have a unit root; the alternative is that at least some are stationary. Under cross-sectional independence,

$$
P=-2\sum_{i=1}^{N}\log p_i \ \sim\ \chi^2_{2N}.
$$

A small combined p-value rejects the all-unit-roots null; it does not establish stationarity for every entity.

The only **result** contains method, deterministic, lags, observations (total effective ADF sample), statistic, degrees_of_freedom, p_value and entity_tests (entity, effective observations, ADF statistic and p-value). If an individual p-value underflows to zero, combined statistic is null to denote the positive-infinite limit and p_value is 0.

Each entity needs at least 4 original observations, positive residual degrees of freedom after lagging and an identifiable design. Constant series, rank failure or an individual test failure reject the entire request. Cross-sectional dependence invalidates the independence-based combined p-value. This entry does not implement LLC, IPS or dependence corrections.

See the [Stata panel unit-root manual](https://www.stata.com/manuals/xtxtunitroot.pdf).
