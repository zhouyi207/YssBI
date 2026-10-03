# 事后多重比较

连接有限数值 response 和逐行对齐、无缺失的 groups。分组标签支持数值、分类、顺序、二元、文本及标识符。至少需要两组，输入行表示独立观测，不适用于重复测量。

equal_variances 默认 true。所有组对使用单因素 ANOVA 合并残差方差 $MSE=\sum_g\sum_i(y_{gi}-\bar y_g)^2/(N-G)$，要求 $N>G$。组 a 与 b 的 $SE=\sqrt{MSE(1/n_a+1/n_b)}$，$t=(\bar y_a-\bar y_b)/SE$ 使用 $N-G$ 自由度。

关闭 equal_variances 时，每组至少两项观测。Welch 使用 $SE^2=v_a+v_b$，$v_g=s_g^2/n_g$，自由度 $df=(v_a+v_b)^2/[v_a^2/(n_a-1)+v_b^2/(n_b-1)]$。每个比较均以 t 分布检验 $H_0:\mu_a=\mu_b$，备择为双侧均值差异。比较标准误为零时无法进行此推断。

adjustment 默认 holm：将 m 个原始 P 值排序后，使用 $p^*_{(i)}=\min(1,\max_{j\le i}(m-j+1)p_{(j)})$。bonferroni 使用 $\min(1,mp)$；none 不校正 P 值。当单个检验有效时，Holm 与 Bonferroni 控制完整组对集合的族错误率。confidence_level 默认 0.95，须严格介于 0 和 1；将 adjusted_p_value 与 $1-c$ 比较。

区间为均值差加减 t 临界值乘标准误。Bonferroni 使用尾部概率 $(1-c)/(2m)$，给出同时区间；Holm 和 none 仍保留名义水平的未校正区间。解释覆盖率前请查看 intervals_adjusted。

result 包含按首次出现排列的 group_labels、各组样本量/均值/标准差、method、adjustment、confidence_level、comparisons、intervals_adjusted。comparisons 表包含 group_a/group_b（group_labels 中从 1 开始的位置）、estimate（a 减 b）、standard_error、degrees_of_freedom、statistic、p_value、adjusted_p_value、lower、upper。节点比较原始组均值，不执行 Tukey 极差检验或协变量调整后的对比。
