# Panel Fit

连接对齐、等长且有限的数值序列：因变量 response、自变量 predictors、实体 entity 和时间 time。实体与时间的组合必须唯一；估计前按实体、时间排序。

参数包括估计器、效应维度、截距和标准误：

- fixed_effects、lsdv、random_effects 支持 entity、time、two_way。
- first_difference 只支持 entity，按原始整数时间的相邻期差分。
- between 支持 entity 或 time，仅接受 nonrobust。
- maximum_likelihood 支持三种效应维度，仅接受 nonrobust。
- lsdv 必须包含截距。其他受支持估计器可选 nonrobust、HC0–HC3 或 cluster。

无效组合或估计失败会明确报错。输出 model 保留系数、协方差及方法推断/分组统计，并输出明确标记的估计尺度 fitted/residuals 数列；其源行及预测含义见 Panel Summary。
