# 在正式打包分发前，可以等渲染完毕再显示窗口
# 在目前的开发环境中，不要这样做，可以取消 debug

# DOLIST

## 2026.02.27

- [x] 竞态条件导致的，应该使用 CQRS 架构，避免直接获取数据：~~打包后 addGraph: Graph "8c010862-6023-4fb5-96f7-ea4ebdfecfc0" already exists 还是会出现，说明在dev阶段就调用了两次~~
- [x] 在 dataseries 结构体中添加了新的字段来显示 dummy 信息，~~同时 ols 在这里并没有处理 string 类型变量，string 类型变量应该 dummy 后然后去剔除一个设置哑变量参与到回归中去，同时在这里需要备注 dataseries 剔除哪个哑变量，以及其属性是否是 时间属性，如果是时间属性的话那么公式中应该要用 t 表示；~~
- [x] 之前是使用 draggedPin.type 这个残缺的来获取 datatype 的，现在使用了 typedisplay 来精确重建 ~~ouput pin 拉出后出现的节点菜单是正常的，input pin 拉出后出现的节点菜单好像有点不正常~~
- [x] 都使用了 Input 框来显示，并对数值类型和字符串类型做了一些限制，~~pin input 需要处理 还有常数节点~~

## 2026.02.28

- [x] ~~ols 功能我希望分为两点，一个是 ols，一个是 ols_summary；ols_summary 是对 ols 能做出的所有报告的总结；~~
- [x] ~~ols 只返回 ols model, 而 ols_summary 返回 ols result, ols result 包含了 ols~~
- [x] 在这里添加了四个库 katex react-markdown rehype-katex remark-math @types/katex ~~ols summary view 可以添加一个公式~~
- [x] 在这里添加了两个库 @types/d3 d3 ~~ols summary view 中再添加一个残差图呗，就简单的 scatter 图就好了使用 d3 吧，拓展性太强了比 chartjs 要好很多~~
- [x] 修复了测试报错情况，~~测试目前由于 execution::new() 修改了导致测试报错~~
- [x] 获取 unique name 迁移到后端，~~在快速创建的时候，可能由于后端的数据并没有反应到前端，导致前端数据不及时从而创建两个相同的 event~~
- [x] 给 dataviewWindow 添加了 column stats 和 overview ，并使用了直方图和条形图进行处理
- [x] 后端依旧使用 path 作为 title，已修复， ~~editorviewer 导入 database 后 title 和 dataviewer 不同步~~
- [x] 给 editorviewer 的 data sidebar 中的 database 添加一个 eye 图标，点击就可以打开 data viewer
- [x] 修复 dataviewer 数据表格的显示问题，可以显示足够多的列，使用 react-virtual 行也虚拟化了 ~~无法显示足够多的列~~

## 2026.03.03

- [x] 添加 plot 支持
- [x] 添加 随机分布 支持 ~~随机分布的随机样本，设置数量出现随机样本~~
- [x] 添加对 dataseries 运算符的支持 ~~catelog math 节点应该是 any~~

# TODOLIST

OneOf 还是使用 Restriction + TypeVar 来做类型推断之类的东西？

oneof 类型连接其中一个类型的 pin 的时候，会变成该类型；（这个效果是好还是不好？很难知道）

我觉得 plot 可以学习 seaborn 来进行参数选择和绘制

catelog 中的 plot（尤其） 和 distribution 中的内容需要大量重构

同时 plot 可以使用数据节点，然后数据节点需要使用 plot 的 show 节点才可以展示，然后 show 节点可以结合多张图的数据节点，在新的窗口中配置如何结合的信息。

复制粘贴节点以及项目的保存不够完善

直方图有问题，其显示将 null 值当作了 0 在图中处理，正常应该忽略

打开 dataviewer 后，在 editorviewer 中导入数据，dataviewer 并没有更新；

类型推断系统不够完善

type infer 使用 dirty 的形式推测，不要每次都推测全量类型

function, macro 功能添加（感觉没必要区分 function 和 macro ）

ols 节点的 evaltor 逻辑有巨大的优化空间

不如纯 data 节点在连接的时候就进行计算就好了

执行动画有一点bug，节点在获取数据执行其他节点的时候本身状态并没有执行完毕但是ui上显示执行完毕了

node 的 tooltip 功能，可以查看节点的信息

为了解决 dummy 问题，例如个体效应和时间效应的哑变量问题，可能需要在 dataseries 中添加额外信息进而去提出多出来的哑变量：计划做一个 add dummy info 节点，下方有个下拉列表或者文本框选择需要设置 dummy 的信息，仅对 dataseries 为 string 的信息管用。

面板数据，可能需要对 dataframe 数据类型也加上 info，来表示个体信息和时间信息 dataseries

未来可视化需要加上地图，地图应该使用下载的方式下载到数据库中，下载完毕后才能使用；

settingview.tsx 中，要注意区分，首先 dataframe 是必须要设置形状和颜色的，然后 array, dataseries 等复合类型应该是控制形状，基础类型控制颜色，同时基础类型应该是相同的形状，any 和 受限制的 any 应该是颜色，应该区分 all any, base any, part any

工具变量法：2sls

dataframe 抽样方法，是在 ols 配置还是 dataframe 层面配置呢

参数网格 -》 数据变换（异常值检验，对数变换等等操作） -》 检验结果 -》 存储

ols_summary 打开 ols_result_viewer 并返回 ols_result, 这里面存储了一些统计模型信息可以使用节点进行提取；这些节点应该是使用类似于函数或者其他功能的注册的方式而不是定义，不然后续结构体太多了这里会爆炸

ols model 可以引申出一个新的节点 predict，这个节点可以使用 endog, exog 两个玩意获得拟合值，然后真实值 - 拟合值可以得到残差。这是基本操作不应该删除

deserializeGraph 这个玩意是干嘛的，好多地方都没必要用他，感觉好卡

下面这玩意在软件退出时保存了两次

[12:43:12.734][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.734][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.735][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.736][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.756][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.757][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.757][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.758][BE][DEBUG] [APP] Settings saved successfully via backend


### 二、Pin 右键类型收窄

**目标**：用户可以右键点击 `OneOf` 类型的 Pin，将其收窄为某个具体成员类型。

**结构**：

```
PinInstance 新增字段:
  type_narrowing: Option<DataType>
```

**需要改动的位置**：

| # | 位置 | 改动内容 |
|---|---|---|
| 1 | `pin_instance.rs` — struct | 添加 `type_narrowing: Option<DataType>` 字段 |
| 2 | `pin_instance.rs` — `from_definition` | 初始化为 `None` |
| 3 | `type_inference_session.rs` — `register_all` | 注册 Pin 类型时，若 `type_narrowing` 有值，用它覆盖定义中的 OneOf |
| 4 | 后端 API | 新增命令：`set_pin_type_narrowing(pin_id, Option<DataType>)` |
| 5 | 前端 Pin 右键菜单 | 检测 Pin 定义是否含 OneOf → 生成收窄选项菜单 + "重置"选项 |
| 6 | 前端 Pin 类型显示 | 收窄后显示具体类型，未收窄显示 `Float64 \| String` |
| 7 | 收窄后触发 | 设置 `type_narrowing` → 重跑类型推断 → 检查已有连线兼容性 |

**优先级链**：`type_narrowing` > 类型推断结果 > Pin 定义默认值


### 六、`VariableSpec` 与 `OLSModel`

**目标**：OLS Model 存储完整的变量编码规格，预测时复用。

**结构**：

```
VariableSpec (enum)
├── Numeric
│   └── name: String
└── Categorical
    ├── name: String
    ├── categories: Vec<String>     // 拟合时的所有 unique 值（有序）
    ├── dropped: String             // 被剔除的参考类别
    └── role: CategoricalRole       // General / Individual / Time

OLSModel (新结构，拟合产物)
├── betas: Array1<f64>
├── has_constant: bool
├── variable_specs: Vec<VariableSpec>   // 按 exog 矩阵列组装顺序排列
├── ...其他拟合统计量
```

**需要改动的位置**：

| # | 位置 | 改动内容 |
|---|---|---|
| 1 | `ols_nodes.rs` | 新增 `VariableSpec` 枚举和 `OLSModel` 结构体 |
| 2 | `ols_nodes.rs` — OLS evaluator | 拟合时构建 `variable_specs`，存入 OLSModel |
| 3 | `info_nodes.rs` | OLS Summary 从 OLSModel 提取信息展示 |

---

### 七、Predict 节点

**目标**：利用 OLSModel 对新数据做预测，分类变量自动编码。

**节点设计**：

```
"Predict" 节点
├── Model:        OLSModel                                    (fixed)
├── Exog:         DataSeries<OneOf([Float64, String])>        (repeatable)
└── Predicted:    DataSeries<Float64>                         (fixed, output)
```

**执行逻辑**：
1. 从 OLSModel 读取 `variable_specs`
2. 按顺序处理每个 Exog 输入：
   - `Numeric` → 直接取 f64 数组
   - `Categorical` → 用 Model 存储的 `categories` + `dropped` 做同样编码
   - 出现 Model 中没见过的类别 → 报错
3. 组装 exog 矩阵（可选加 constant 列）
4. 计算 `predicted = X * betas`
5. 输出 `DataSeries<Float64>`

---