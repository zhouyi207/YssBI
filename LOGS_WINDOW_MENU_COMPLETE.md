# 日志窗口菜单项添加完成

## 概述
在前端菜单栏的 Windows 菜单下添加了"Logs"菜单项，用户可以通过点击该菜单项打开独立的日志窗口。

## 实现内容

### 1. 菜单项添加 (`src/components/Editor/Layout/Menubar.tsx`)

#### 添加 `handleOpenLogs` 函数
```typescript
const handleOpenLogs = async () => {
  try {
    const label = `logs-${Math.random().toString(36).substring(7)}`;
    new WebviewWindow(label, {
      url: "index.html#/logs",
      title: "Logs",
      width: 1000,
      height: 600,
      decorations: false,
      visible: false, // 初始隐藏，待渲染完毕后再显示
    });
  } catch (error) {
    console.error("Failed to open logs window:", error);
    showToast("无法打开日志窗口", "error");
  }
};
```

#### 在 `windowItems` 数组中添加菜单项
```typescript
const windowItems: MenuItem[] = [
  { label: "New Window", onClick: openNewWindow },
  { label: "-" },
  { label: "Split Editor Right", onClick: handleSplitRight },
  { label: "Split Editor Down", onClick: handleSplitDown },
  { label: "-" },
  { label: "Logs", onClick: handleOpenLogs },  // 新增
  { label: "-" },
  { label: "Reset Layout" },
  { label: "Zoom In", shortcut: "Ctrl++" },
  { label: "Zoom Out", shortcut: "Ctrl+-" },
];
```

### 2. 路由配置 (`src/App.tsx`)

#### 导入 LogWindow 组件
```typescript
import { LogWindow } from "./components/LogView/LogWindow";
```

#### 添加窗口类型检查
```typescript
const isLogsWindow = window.location.hash === '#/logs';
```

#### 添加路由处理
```typescript
// 如果是 Logs 窗口，只渲染 LogWindow 组件
if (isLogsWindow) {
  return (
    <ThemeProvider>
      <LogWindow />
    </ThemeProvider>
  );
}
```

### 3. 代码清理 (`src/components/LogView/LogWindow.tsx`)
- 移除了未使用的 `FiX` 导入，修复了 TypeScript 警告

## 使用方式

1. 启动应用程序
2. 点击顶部菜单栏的 "Window" 菜单
3. 选择 "Logs" 菜单项
4. 将打开一个新的独立日志窗口，显示所有应用程序、执行和系统日志

## 窗口特性

- **独立窗口**: 使用 Tauri 的 WebviewWindow 创建独立窗口
- **自定义装饰**: 无系统边框 (`decorations: false`)
- **默认尺寸**: 1000x600 像素
- **初始隐藏**: 窗口创建后初始隐藏，待内容渲染完成后显示
- **唯一标识**: 每个窗口都有唯一的随机标识符，支持打开多个日志窗口

## 技术实现

### 窗口创建模式
采用与 Data Viewer 相同的窗口创建模式：
- 使用 `WebviewWindow` API
- 通过 URL hash 路由 (`#/logs`) 区分窗口类型
- 在 App.tsx 中根据 hash 渲染对应组件

### 路由架构
```
主窗口: index.html (默认)
  └─ MainApp 组件

Plot 窗口: index.html#/plot
  └─ PlotWindow 组件

DataView 窗口: index.html#/dataview
  └─ DataViewWindow 组件

Logs 窗口: index.html#/logs
  └─ LogWindow 组件
```

## 验证结果

✅ TypeScript 编译无错误
✅ 菜单项正确显示在 Window 菜单下
✅ 点击菜单项可以打开日志窗口
✅ 日志窗口正确渲染 LogWindow 组件
✅ 支持打开多个日志窗口实例

## 相关文件

- `src/components/Editor/Layout/Menubar.tsx` - 菜单栏组件
- `src/App.tsx` - 应用入口和路由配置
- `src/components/LogView/LogWindow.tsx` - 日志窗口组件
- `src/store/logStore.ts` - 日志状态管理
- `src/types/logging.ts` - 日志类型定义

## 后续优化建议

1. **快捷键支持**: 可以添加快捷键（如 Ctrl+Shift+L）快速打开日志窗口
2. **窗口状态保存**: 保存日志窗口的位置和大小，下次打开时恢复
3. **单例模式**: 如果不需要多个日志窗口，可以实现单例模式，避免重复打开
4. **窗口通信**: 实现主窗口与日志窗口之间的通信，支持从主窗口控制日志窗口

## 完成时间
2026-02-02
