# 相关图（ACF 与 PACF）

Values 接收至少 4 个有限且非常量的 Numeric 观测，拒绝缺失值。Maximum lag 默认 20，可取 1–40，实际最大阶数为 $\min(L,\lfloor n/2\rfloor-1,40)$。

ACF 与 Durbin–Levinson PACF 从第 1 阶显示，95% 白噪声参考带半宽为 $1.96/\sqrt n$。ACF 另外给出累积 Ljung–Box $Q_k=n(n+2)\sum_{j=1}^k r_j^2/(n-j)$，检验前 $k$ 阶自相关均为零，以 $\chi^2_k$ 计算 P 值；这里不调整拟合参数自由度。PACF 不附加 Q 检验。Result 在结果面板或 Plot 窗口展示双面板。
