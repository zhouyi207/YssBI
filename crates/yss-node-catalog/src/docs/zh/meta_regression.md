# Meta 回归

连接对齐的 effects、正 variances 和至少一个数值 moderators。始终包含截距，设计矩阵须满秩，研究数 $k>p$，其中 $p$ 包含截距。系数使用原始协变量单位及输入列名。
模型 $y=X\beta+u+\epsilon$，以 $\hat\beta=(X'WX)^{-1}X'Wy$ 拟合，$W_{ii}=1/(v_i+\tau^2)$。estimator 默认 paule_mandel，fixed 令异质性方差为零。Paule–Mandel 求解残差 $Q(\tau^2)=k-p$；der_simonian_laird 使用 $\max(0,(Q(0)-k+p)/\operatorname{tr}P)$，$P=W-WX(X'WX)^{-1}X'W$ 在 $\tau^2=0$ 下计算。
异质性字段使用残差自由度 $k-p$，评估未解释的研究差异。研究层面的关联不能证明个体层面的关联或因果效应。

confidence_level 默认 0.95，须严格介于 0 和 1。inference 默认 wald，系数检验为 $H_0:\beta_j=0$ 对 $\beta_j\ne0$，采用正态 Wald 推断；knapp_hartung 将协方差乘以 $Q(\hat\tau^2)/(k-p)$，检验与区间使用 $t_{k-p}$，单纯合并时 $p=1$。残差尺度小于 1 时其区间可能更窄。方差推断以已拟合 τ² 为条件。

result 包含 studies、estimator、inference、coefficients（估计值、标准误、统计量、P 值、区间）、covariance、residual_degrees_of_freedom、residual_q、heterogeneity、prediction_interval。heterogeneity 报告固定权重 Q，以及共同效应对异质性备择的卡方 P 值（$df=k-p$）、I²、H²、τ²。studies 表包含 study、effect、variance、standard_error、lower、upper、weight（和为 1）、fitted、residual。至少需要两项研究，且设计矩阵满秩。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
