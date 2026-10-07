# ROC Curve

Labels is a Binary series and Scores is an aligned Numeric score series. Higher scores indicate the positive class. Both classes must be present; missing and nonfinite inputs are rejected.

Thresholds descend by score; tied scores enter together. Plots false-positive against true-positive rates. AUC is the trapezoidal area of the complete ROC, giving tied positive/negative pairs half credit. Curves over 2048 points are sampled for display while AUC uses all observations. The diagonal is the random-ranking reference.

Open the result output in a workbench result panel or a separate Plot window.
