# 节点拖动性能优化

## 问题描述

节点拖动时，连接线的更新与节点移动不同步，出现明显的延迟和卡顿。节点移动时有明显的"慢半拍"感觉。

## 根本原因

1. **CSS Transition 导致的延迟** ⚠️ **主要原因**
   - 节点使用了 `transition-all duration-200` 类
   - 这会对所有 CSS 属性（包括 transform）应用 200ms 的过渡动画
   - 拖动时每次位置更新都会触发 200ms 的动画，导致严重的延迟感

2. **React 渲染延迟**
   - 节点位置通过 React 状态管理
   - 状态更新 → 组件重渲染 → DOM 更新
   - 这个过程至少需要一帧的时间（16.67ms @ 60fps）

3. **连接线绘制依赖节点位置**
   - EdgesLayer 通过 `getPinWorldPos()` 获取 Pin 的世界坐标
   - Pin 坐标基于节点的 DOM 位置
   - 如果节点 DOM 更新慢，连接线就会滞后

## 解决方案

### 1. 🔥 移除 transform 的过渡动画（关键修复）

**文件**: `src/components/Editor/Nodes/Node.tsx`

将 `transition-all` 改为只对特定属性应用过渡：

```typescript
// ❌ 之前：对所有属性应用过渡（包括 transform）
className="... transition-all duration-200"

// ✅ 现在：只对视觉属性应用过渡，排除 transform
className="... " // 移除 transition-all
style={{
  // ...
  transition: "border-color 200ms, box-shadow 200ms, background 200ms",
  // ...
}}
```

**效果**:
- transform 立即生效，无延迟
- 边框、阴影、背景仍有平滑过渡
- 拖动体验从"慢半拍"变为"即时响应"

### 2. 直接操作 DOM（节点移动）

**文件**: `src/components/Editor/Hooks/useCanvasInteraction.ts`

在拖动时，绕过 React 状态，直接更新节点的 `transform` 样式：

```typescript
// 🆕 直接操作 DOM 以获得即时反馈
sIds.forEach(id => {
    const nodeEl = document.querySelector(`[data-node-id="${id}"]`) as HTMLElement;
    if (nodeEl) {
        // 获取当前 transform
        const currentTransform = nodeEl.style.transform;
        const match = currentTransform.match(/translate3d\(([-\d.]+)px,\s*([-\d.]+)px,\s*([-\d.]+)px\)/);
        if (match) {
            const currentX = parseFloat(match[1]);
            const currentY = parseFloat(match[2]);
            const newX = currentX + dx;
            const newY = currentY + dy;
            // 立即更新 DOM
            nodeEl.style.transform = `translate3d(${newX}px, ${newY}px, 0)`;
        }
    }
    // 同时更新状态（用于持久化）
    useNodeStore.getState().updateNodePosition(tid, id, dx, dy);
});
```

**优点**:
- 节点位置立即更新，无需等待 React 重渲染
- 保持 60fps 的流畅度
- 状态仍然会更新，确保数据一致性

### 3. 持续动画循环（连接线绘制）

**文件**: `src/components/Editor/Canvas/EdgesLayer.tsx`

在拖动时启动持续的动画循环，每帧重绘连接线：

```typescript
// 持续的动画循环（仅在需要时运行）
const animate = useCallback(() => {
    drawAllEdges();
    if (isAnimatingRef.current) {
        rafRef.current = requestAnimationFrame(animate);
    }
}, [drawAllEdges]);

// 监听手势状态，在拖动时启动动画循环
useEffect(() => {
    const unsubGesture = useGestureStore.subscribe((state) => {
        const currentGesture = state.gesture;
        if (currentGesture && (currentGesture.type === "drag" || currentGesture.type === "pan")) {
            startAnimation();
        } else {
            stopAnimation();
            drawAllEdges(); // 停止后绘制最后一帧
        }
    });
    // ...
}, [startAnimation, stopAnimation, drawAllEdges]);
```

**优点**:
- 拖动时每帧都重绘连接线
- 连接线始终跟随节点位置
- 非拖动时停止循环，节省性能

## 性能对比

### 优化前
- 节点更新：React 状态 → 重渲染 → DOM 更新 + 200ms 过渡动画（~216-232ms）
- 连接线更新：等待状态变化通知 → 单次重绘（延迟 1-2 帧）
- 结果：明显的"慢半拍"和不同步

### 优化后
- 节点更新：直接 DOM 操作，无过渡动画（<1ms）
- 连接线更新：持续动画循环（每帧重绘）
- 结果：完全同步，流畅的 60fps，即时响应

## 关键改进

### 问题根源
200ms 的 CSS 过渡动画是导致"慢半拍"的主要原因：
- 每次拖动更新都会触发 200ms 的动画
- 用户移动鼠标时，节点需要 200ms 才能到达目标位置
- 这导致节点始终落后于鼠标指针

### 解决方案
只对视觉效果（边框、阴影、背景）应用过渡，排除位置变换：
- transform 立即生效，无延迟
- 视觉效果仍然平滑过渡
- 拖动体验完全改变

## 数据流

```
用户拖动鼠标
    ↓
onPointerMove 事件
    ↓
计算位移 (dx, dy)
    ↓
┌─────────────────────┬─────────────────────┐
│  直接操作 DOM        │  更新 React 状态     │
│  (即时反馈)          │  (持久化)            │
│  ↓                   │  ↓                   │
│  节点立即移动        │  useNodeStore        │
│  (<1ms, 无动画)      │  (异步)              │
└─────────────────────┴─────────────────────┘
    ↓
连接线持续重绘 (每帧)
    ↓
流畅的拖动体验 (60fps)
```

## 注意事项

1. **DOM 和状态的一致性**
   - DOM 操作是临时的，用于即时反馈
   - 状态更新是持久的，用于数据保存
   - 两者必须同时进行

2. **CSS Transition 的使用**
   - 只对视觉属性应用过渡
   - 避免对 transform、top、left 等位置属性应用过渡
   - 拖动时需要即时响应，不能有延迟

3. **性能考虑**
   - 持续动画循环仅在拖动时启用
   - 非拖动时使用事件驱动的单次重绘
   - 避免不必要的性能开销

4. **浏览器兼容性**
   - `transform: translate3d()` 触发 GPU 加速
   - `requestAnimationFrame` 确保与浏览器刷新率同步
   - 所有现代浏览器都支持

## 测试

### 测试步骤
1. 创建多个节点并连接它们
2. 拖动节点观察连接线
3. 快速拖动测试流畅度
4. 拖动多个选中的节点

### 预期结果
- ✅ 节点移动流畅，无延迟，无"慢半拍"
- ✅ 节点立即跟随鼠标指针
- ✅ 连接线完全跟随节点
- ✅ 无明显的卡顿或撕裂
- ✅ 60fps 的帧率

### 性能指标
- 节点位置更新：<1ms（无过渡动画）
- 连接线重绘：<5ms（取决于连接数量）
- 总帧时间：<16ms（60fps）

## 未来改进

1. **虚拟化**
   - 只渲染可见区域的节点和连接线
   - 大型图（>100 节点）的性能优化

2. **Web Workers**
   - 将连接线计算移到 Worker 线程
   - 进一步减少主线程负担

3. **Canvas 优化**
   - 使用离屏 Canvas
   - 分层渲染（静态层 + 动态层）

## 相关文件

- `src/components/Editor/Nodes/Node.tsx` - 🔥 移除 transform 的过渡动画
- `src/components/Editor/Hooks/useCanvasInteraction.ts` - 节点拖动逻辑
- `src/components/Editor/Canvas/EdgesLayer.tsx` - 连接线绘制
- `src/components/Editor/Store/useGestureStore.ts` - 手势状态管理
- `src/components/Editor/Store/useNodeStore.ts` - 节点状态管理
