# Panel Summary

连接 Panel Fit 产生的 `model`。Configure 中的 `model_summary`、`coefficient_table`、`effects_statistics` 和 `estimator_statistics` 分别控制模型概览、系数、效应及估计器统计，默认全部开启。估计器统计随拟合方法变化，例如 RE 的 Wald 或 MLE 的似然统计。

`result` 和 `report` 只包含所选内容，复用已拟合系数、协方差及分组统计，不重新拟合，不需要原始输入。
