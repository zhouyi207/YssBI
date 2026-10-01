# 灰色预测 GM(1,1)

连接按时间排序、等间隔的数值 `series`，至少四条严格为正的有限观测；拒绝缺失值。`ts_horizon=10` 为正整数，不设固定数据行数上限。

累加序列为 $X_k=\sum_{i=0}^k y_i$，用最小二乘估计

$$
y_k=-a\frac{X_{k-1}+X_k}{2}+b,\qquad k=1,\ldots,n-1.
$$

时间响应为 $\widehat X_k=(y_0-b/a)e^{-ak}+b/a$，还原预测为 $\widehat y_k=\widehat X_k-\widehat X_{k-1}$。$a=0$ 时使用连续极限；设计矩阵须满秩。

`parameters` 给出 `development_a` 与 `input_b`。`fitted`、`residuals` 与原始行对齐，首行仅用于初始化，故设为 null。`forecasts[0]` 是样本后的第一期预测。`innovation_variance` 为还原尺度的均方误差，不代表随机灰色模型的方差估计；似然、AIC/BIC、预测区间为空，不包含假设检验。

GM(1,1) 假定近似指数变化。正值条件不等于模型适用，应检查还原残差、级比和留出预测。本节点不自动平移非正数据，也不搜索预处理方案。
