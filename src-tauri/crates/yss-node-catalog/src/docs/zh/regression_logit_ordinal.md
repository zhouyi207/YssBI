# 有序Logit

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

比例优势的有序 Logit 不另设截距。有序元数据决定顺序，普通数值响应按分数升序，不推定文本顺序；未观测声明级别被省略。至少两个观测类别且观测数须大于参数数目；500 次迭代、容差 1e-7。`cut[c|c+1]` 为递增的实际阈值，并转换 Delta 协方差。正的斜率使概率偏向高类别。概率/预测标签按 `categories` 排列，数值残差及 RSS/RMSE 不适用。Wald 推断采用观测信息和标准正态参考；常数自变量/分离/信息矩阵奇异时失败。

$$
\operatorname{logit}P(Y\le c\mid x)=\theta_c-x^T\beta,\quad\theta_0<\theta_1<\cdots<\theta_{K-2}.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.miscmodels.ordinal_model.OrderedModel.html)
