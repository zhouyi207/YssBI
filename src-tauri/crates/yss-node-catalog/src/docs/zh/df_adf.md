# DF / ADF

连接按时间顺序排列的有限数值序列。lags 可为 0（DF）或正整数（ADF），regression 可选 none、constant、trend。

原假设为存在单位根。result 为结构化结果，包含检验统计量、p 值、临界值、实际滞后阶数与样本数，不输出已拟合模型。样本不足或估计失败会报错。
