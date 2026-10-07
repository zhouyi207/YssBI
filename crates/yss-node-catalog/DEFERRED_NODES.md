# 暂缓节点

> Status: Planned
> Scope: 本轮节点补齐中按用户要求暂缓的复杂算法及重新开展工作的前提
> Canonical owners: Catalog inventory 保留节点身份；KernelRegistry 决定实际可执行性
> Update when: 新增暂缓判断、实现所列节点或改变本轮范围时

用户已将范围调整为优先补齐可可靠实现的节点，复杂的机器学习、神经网络等暂缓，
并要求记录原因。下列节点继续显示为不可用，不用其他算法或空结果替代。
未列在此文档中的待实现节点仍属于本轮补齐范围。

## 机器学习类别

本轮按用户举例暂缓整个机器学习类别。共同缺口包括训练/验证数据的显式划分、
训练时预处理复用、可复用预测模型、可重复训练及独立数值与预测验证。
简单分类器和聚类算法本身未必复杂，但同属本轮暂缓的机器学习类别；
后续可以先独立评估 KNN、朴素贝叶斯和聚类。

| 节点 ID                                               | 方法       | 后续实现要点                                   |
| ----------------------------------------------------- | ---------- | ---------------------------------------------- |
| yssbi.statistics.ml.adaboost                          | AdaBoost   | 基学习器、样本权重更新、多分类口径与预测       |
| yssbi.statistics.ml.apriori                           | Apriori    | 事务编码、频繁项集搜索、规则度量和输出规模控制 |
| yssbi.statistics.ml.catboost                          | CatBoost   | 有序目标编码、有序提升及避免目标泄漏           |
| yssbi.statistics.ml.decision_tree                     | 决策树     | 分裂准则、剪枝、缺失值和分类/回归模型          |
| yssbi.statistics.ml.extra_trees                       | 极端随机树 | 随机阈值、集成训练及随机种子语义               |
| yssbi.statistics.ml.gbdt                              | GBDT       | 损失函数、梯度提升、停止条件与预测             |
| yssbi.statistics.ml.knn                               | KNN        | 训练尺度复用、距离、邻居并列与预测             |
| yssbi.statistics.ml.lightgbm                          | LightGBM   | 直方图与叶优先生长，不能用普通 GBDT 冒充       |
| yssbi.statistics.ml.naive_bayes                       | 朴素贝叶斯 | 连续/计数/二元分布、平滑和概率输出             |
| yssbi.statistics.ml.neural_network                    | 神经网络   | 网络结构、反向传播、优化器、收敛与推理         |
| yssbi.statistics.ml.random_forest                     | 随机森林   | 自助采样、随机特征、集成预测与袋外评估         |
| yssbi.statistics.ml.svm                               | 支持向量机 | 核函数、约束优化、支持向量及预测               |
| yssbi.statistics.ml.xgboost                           | XGBoost    | 二阶提升、正则化、树结构与稀疏缺失值语义       |
| yssbi.statistics.multivariate.clustering              | 聚类       | 明确 K-means 等方法、初始化、尺度及聚类模型    |
| yssbi.statistics.multivariate.hierarchical_clustering | 分层聚类   | 距离与连接规则、树结构及按层级切分             |

## 潜变量与约束优化

现有回归、矩阵分解和无约束优化不能直接替代以下模型所需的识别规则或约束求解。
逐方程 OLS、普通加权评分或无约束最小化都不足以实现这些节点的完整含义。

| 节点 ID                             | 方法             | 暂缓原因与前提                                                                      |
| ----------------------------------- | ---------------- | ----------------------------------------------------------------------------------- |
| yssbi.statistics.sem.cfa            | 验证性因子分析   | 需要潜变量测量模型、尺度识别、固定/自由载荷与误差协方差约束、拟合指数和信息矩阵验证 |
| yssbi.statistics.sem.structural     | 结构方程模型     | 还需结构路径、间接效应、模型识别和联合协方差结构估计，不能用逐方程回归替代          |
| yssbi.statistics.decision.dea       | 数据包络分析     | 缺少带约束的线性规划求解与可行/不可行/无界状态契约，还需明确规模报酬与投入/产出方向 |
| yssbi.statistics.decision.sbm       | 松弛变量效率模型 | 需要分式规划变换、松弛约束和效率前沿求解；不能用 DEA 径向分数冒充                   |
| yssbi.statistics.decision.malmquist | Malmquist 指数   | 依赖多个跨期 DEA 距离问题及可比决策单元、期间对齐和前沿分解                         |
| yssbi.statistics.decision.bwm       | 最好最差赋权     | 需要明确线性或原始比值模型，以及极小极大约束求解、一致性口径和最优解验证            |

## 多数据集与通用重抽样工作流

这些入口需要新的执行和结果组合能力。现有单一统计方法内部的 Bootstrap 能力继续保留，
但不能据此把通用重抽样节点标记为已实现。

| 节点 ID                              | 方法           | 暂缓原因与前提                                                                           |
| ------------------------------------ | -------------- | ---------------------------------------------------------------------------------------- |
| yssbi.dataframe.impute.multiple      | 多重插补 MI    | 需要可重现的多份插补数据、估计任务调用、组内/组间方差和 Rubin 合并推断，不只是多次填均值 |
| yssbi.dataframe.impute.mice          | 链式方程插补   | 在 MI 基础上需要按变量类型的条件模型、随机参数抽样、链迭代与收敛诊断                     |
| yssbi.statistics.inference.bootstrap | 通用 Bootstrap | 需把可调用统计任务与行/聚类重抽样设计接入执行器，并定义重拟合失败、种子及结果汇总        |
| yssbi.statistics.inference.jackknife | 通用 Jackknife | 需可调用统计任务、删一/删组样本构造、伪值和合并推断，不能默认为任意节点都可安全重执行    |

这些节点后续应沿 Catalog → Kernel → SCI Runtime → SCI → Linalg 的现有依赖方向实现。
训练模型、计算结果与图表数据使用中立契约；不把训练逻辑放入前端或目录注册代码。
