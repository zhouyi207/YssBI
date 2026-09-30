# Discriminant analysis

Provide aligned training **Class** labels and 1–16 numeric **Training variable** series. Labels accept materializable base scalars and are grouped exactly by value. Missing labels are rejected. Support 2–8 classes with at least 2 observations each; relational training inputs must share a row domain.

Optional **New variable** inputs must match training variables in count, order and meaning. New observations may come from another row domain, but their columns must align internally. Without new data, predictions classify training observations. Missing numbers and constant training variables are rejected.

**Discriminant model** defaults to `linear`, using pooled within-class sample covariance; `quadratic` uses class-specific sample covariance. **Class priors** defaults to `empirical` training proportions, with `equal` available. **Covariance shrinkage** defaults to 0 in [0,1]. Variables are first standardized using whole-training sample standard deviations, then

$$\Sigma_\alpha=(1-\alpha)\Sigma+\alpha\frac{\operatorname{tr}(\Sigma)}{p}I.$$

Shrunk covariance must be positive definite; singular designs do not silently use a pseudoinverse. LDA assumes common class covariance, while QDA permits differences. Both use approximately multivariate normal within-class models.

Classification maximizes the Gaussian discriminant score

$$\delta_g(x)=-\tfrac12\left[(x-\mu_g)^T\Sigma_g^{-1}(x-\mu_g)+\log|\Sigma_g|\right]+\log\pi_g.$$

$\mu_g$ is the class mean, $\pi_g$ its prior and $\Sigma_g$ the covariance, shared in LDA. Ties choose the class first encountered in training rows. Classification does not introduce a null hypothesis or p-value.

**Result** gives original labels, counts/priors, original-unit class means, training accuracy and an actual-by-predicted training confusion matrix. These are not cross-validated generalization estimates. **Predicted class** is a connectable series preserving original labels/semantics, ordered like new data or training data.

Reference: [scikit-learn LDA/QDA](https://scikit-learn.org/stable/modules/lda_qda.html).
