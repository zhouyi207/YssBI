# 杠杆值

连接 OLS 或 WLS 的 `model`，输出每个拟合观测在自变量空间中的杠杆值。无配置参数，保留拟合观测顺序；当前不支持 GLS。

## 计算公式与含义

设 $X$ 为 $n\times k$ 满列秩设计矩阵，$x_i'$ 为其第 $i$ 行，$k$ 包括已有截距。OLS 的帽子矩阵为

$$
H=X(X'X)^{-1}X',\qquad h_i=H_{ii}=x_i'(X'X)^{-1}x_i.
$$

WLS 使用原正精度权重 $W=\operatorname{diag}(w_i)$：

$$
H_w=W^{1/2}X(X'WX)^{-1}X'W^{1/2},
\qquad h_i=w_i x_i'(X'WX)^{-1}x_i.
$$

在这些条件下 $0\leq h_i\leq1$、$\sum_i h_i=k$，平均杠杆值为 $k/n$。值较大表示该行对拟合值具有较大潜在影响。它不使用该行残差大小，因此高杠杆不等同于响应异常或实际高影响点。帽子矩阵诊断见 [statsmodels OLSInfluence](https://www.statsmodels.org/dev/generated/statsmodels.stats.outliers_influence.OLSInfluence.html)。

## 输出与判读

`result` 与 `report` 相同：`test=leverage`，内层 `result` 为按拟合行顺序排列的 $h_i$ 数组。这里没有原假设、备择假设或 p 值；也不自动标记、删除观测。

例如 $n=100$、$k=5$ 时平均杠杆值为 0.05。将 $2k/n=0.10$ 或 $3k/n=0.15$ 作为检查起点是经验做法，不是正式检验的临界值。需结合残差、数据录入情况和变量范围判断是否有实际影响。

要求非空、满列秩的原设计；WLS 权重必须有限且为正。奇异矩阵或非有限结果会失败。顺序对应的是模型实际使用的样本；若上游筛选或重排过数据，不能直接按未经处理的原表行号匹配。
