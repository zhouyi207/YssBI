# Between-study variance

Connect effects and positive variances for at least two independent studies. estimator defaults to paule*mandel, solving $Q(\tau^2)=k-1$ with $w_i=1/(v_i+\tau^2)$. der_simonian_laird uses $\hat\tau^2=\max(0,(Q(0)-k+1)/C)$, $C=\sum1/v_i-\sum(1/v_i)^2/\sum1/v_i$. Both truncate to zero when $Q(0)\le k-1$.
result contains tau_squared on the squared effect scale, plus q, degrees_of_freedom, p_value, i_squared_percent and h_squared. The P value tests common effects using $Q(0)\sim\chi^2*{k-1}$ against heterogeneity; it is not a boundary-corrected test of τ². This node supplies a point estimate, without a τ² confidence interval.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
