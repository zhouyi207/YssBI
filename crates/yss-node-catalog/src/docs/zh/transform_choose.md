# 条件选择

连接 Binary 条件及 When True、When False 两个值。分支可以是同语义类型的标量或已对齐数列，物理表示必须兼容。标量在条件数列的行域内广播。

条件为真时选择 When True，为假或 Null 时选择 When False。数列输入须等长并按当前位置逐项对应，可混合数据库与内存数列；分支值为 Null 时仍为 Null。
