# Decompose DataFrame

将 **DataFrame** 按列拆分为多个 **DataSeries** 输出。连接 **DataFrame** 后，根据输入表 schema 动态生成与列数对应的输出 pin。

## 用法

先连接 **DataFrame**，列输出 pin 会自动出现。将各列接到变换、比较或“组装数据帧”等节点。输出 pin 名称与源列名一致，元素语义类型随源列解析。
