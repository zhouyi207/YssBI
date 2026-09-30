# Kendall 协调系数 W

每个 **Rater** 输入为对齐的 Numeric 或 Ordinal 列，每行对应同一个对象。至少两个对象、两位评定者，最多 64 列；Ordinal 按各自声明的等级排序。缺失评分会报错。

对每位评定者跨对象取秩，并列值取平均秩。设有 m 位评定者、n 个对象，各对象秩和为 $R_i$，$T=\sum_j\sum_g(t_{jg}^3-t_{jg})$，则

$$W=\frac{12\sum_i[R_i-m(n+1)/2]^2}{m^2(n^3-n)-mT}.$$

W 在 0 与 1 之间，始终修正并列。在无协调性的原假设下，$\chi^2=m(n-1)W$ 近似服从自由度 n-1 的卡方分布；很小样本时该近似较弱。所有评定者都给出常量排序时 W 未定义，会报错。

`result` 含 W、对象/评定者数、并列修正和卡方推断。本节点计算多评定者协调性。

参考：[R Kendall W](https://search.r-project.org/CRAN/refmans/irr/html/kendall.html)。
