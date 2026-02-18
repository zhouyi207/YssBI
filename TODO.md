# 在正式打包分发前，可以等渲染完毕再显示窗口
# 在目前的开发环境中，不要这样做，可以取消 debug


# TODOLIST

首先是节点创建等等东西，然后将前端序列化做好：支持复制粘贴模式：目前复制粘贴仍旧有问题

接着便是pin 之间的连接




前端状态管理 api <---> 通信

dataframe 接入



节点和连接的序列化和反序列化都是要做的，前者可以将内容输出给前端和保存，后者可以复制粘贴节点

列表


复制粘贴节点需要什么？？？

首先是节点信息，比如节点类型，节点的 pins

然后是节点的相对位置


目前连接状态的结构体有没有必要修改，from-to 连接应该是无向的


而 DTO 应该只包含节点的必要信息，比如节点类型，节点的 pins，以及节点的相对位置


name category 应该在 node definition 创建的时候赋值，这样就可以计算出 node_type


缓冲层和存储层，主要用于大型数据计算 —— 目前架构好像还是比较好加的，先处理前端先

undo/redo 也是比较好加的，可以处理完毕后再进行添加



deserializeGraph 这个玩意是干嘛的，好多地方都没必要用他，感觉好卡


## node_instance.rs 设计

3.3 order 与 pin_ids 的冗余

- PinInstance 有 order: PinOrder
- NodeInstance 有 pin_ids: Vec<PinId>
- 顺序信息在两个地方都有，存在冗余
- 若 pin_ids 顺序是唯一真相来源，PinInstance.order 可考虑弱化或移除

3.4 动态 Pin 支持不足

- NodeMetaData.supports_dynamic_pins 已存在，但 from_definition 只处理静态 pins
- 动态添加/删除 pin 的逻辑尚未在此体现，后续扩展时需要额外设计