# 复杂抽样线性回归

连接 Y 响应和 weights 抽样权重，可连接一列 strata 层标签及一列 clusters 初级抽样单元（PSU）标签。

未提供 strata 时视为一层；未提供 clusters 时每行是一个独立 PSU。不同层内相同的 PSU 标签分别识别。列须对齐且无缺失，数值有限，抽样权重严格为正。不自动删除记录。

使用单阶段、有放回近似的 Taylor 线性化方差，不包含有限总体修正、多阶段设计、复制权重或子总体分析。第 h 层有 m_h 个 PSU，t_hj 为该 PSU 的得分之和，则方差汇总各层 `m_h/(m_h−1) × Σ(t_hj−层均值得分)²`；回归使用外积矩阵并左右乘信息矩阵逆。设计自由度为 PSU 总数减层数。

`lonely_psu=fail` 默认拒绝单 PSU 层；仅当该层确为必选且无未计入的后续抽样阶段时使用 `certainty`，将其方差贡献设为零。整体设计自由度仍须为正。

内部将权重缩放到均值 1；统一乘常数不改变估计或标准误。设计摘要保留原始权重和、范围、Kish 有效样本量 `(Σw)²/Σw²`。`weight_design_effect=n/Kish有效样本量` 只反映权重不等，不是包含分层、聚类影响的完整设计效应。无固定行数上限。

连续响应，Gaussian 恒等链接伪似然，点估计等于加权最小二乘。

追加数值 X₁, X₂, …；`constant=true` 默认包含截距。每个设计须满秩，观测数须大于回归参数数目。`max_iterations` 默认 500（正整数），`tolerance` 默认 1e-7，范围 1e-12 至 0.01，未收敛报错。

**result** 返回系数、抽样设计 sandwich 协方差、迭代情况和 `details.design`；`factor_names` 只对应 X₁, X₂, … 顺序。k 为回归参数数目，系数推断自由度为设计自由度 + 1 − k，须为正。零系数的双侧统计量为估计除以设计标准误，使用 t 分布及 95% 区间。不使用普通 GLM 模型方差或似然 AIC/BIC。

**observations** 分页表保留全部 `observation, response, fitted, residual`，残差为响应减响应尺度的拟合均值，行号从 1 开始。

[参考：survey::svyglm](https://r-survey.r-forge.r-project.org/pkgdown/docs/reference/svyglm.html)
