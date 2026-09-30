# 杠杆值

连接 OLS 或 WLS 的 `model`，输出每个拟合观测的杠杆值。无配置参数；不支持 GLS。

## 计算公式

$$
h_i=x_i'(X'X)^{-1}x_i\quad\text{(OLS)},
\qquad h_i=w_i x_i'(X'WX)^{-1}x_i\quad\text{(WLS)}.
$$

$X$ 为原设计矩阵，$x_i'$ 为第 $i$ 行，$W$ 为原精度权重组成的对角矩阵。杠杆值衡量观测在自变量空间中的潜在影响，不等同于残差异常或实际影响程度。

## 输出与使用说明

`result` 为结构化结果，`test=leverage`，内层 `result` 为按拟合行顺序排列的 $h_i$ 数组，不输出 p 值或自动删除观测。

满列秩时杠杆值之和为设计列数 $k$，平均值为 $k/n$。结合残差判断高杠杆观测；匹配原表时须沿用模型实际样本的行顺序。奇异设计无法计算。

方法细节：[杠杆值诊断](https://www.statsmodels.org/dev/generated/statsmodels.stats.outliers_influence.OLSInfluence.html)。
