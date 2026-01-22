import { useRef, useState, useEffect, useCallback, useMemo, useLayoutEffect } from "react";
import { Node } from "../Nodes/Node";
import { Pin } from "../Types/nodes";
import { useDrag } from "../Context/DragContext";
import { useCanvas } from "../Context/CanvasContext";
import { useTheme } from "../Context/ThemeContext";
import { useViewportStore } from "../Store/useViewportStore";
import { useNodeStore } from "../Store/useNodeStore";
import { createNodeFromTemplate } from "../Utils/nodeUtils";
import { createInternalNode } from "../Utils/internalNodes";
import HUD from "./HUD";
import NodePalette from "../Nodes/NodePalette";
import { VscRunAll } from "react-icons/vsc";
import { drawEdge } from "../Edges/Edge";

/* ================= Canvas Components ================= */

const GRID = 40;

const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

const ViewportGrid = ({ groupId }: { groupId: string }) => {
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // 零 React 重绘：直接订阅 Store 并同步 DOM 样式
    return useViewportStore.subscribe(state => {
      const canvas = state.viewports[groupId] || DEFAULT_VIEWPORT;
      const el = gridRef.current;
      if (el) {
        el.style.backgroundSize = `${GRID * canvas.scale}px ${GRID * canvas.scale}px`;
        el.style.backgroundPosition = `${canvas.x}px ${canvas.y}px`;
      }
    });
  }, [groupId]);

  const initial = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
  return (
    <div
      ref={gridRef}
      className="absolute inset-0 pointer-events-none"
      style={{
        backgroundImage: `
          linear-gradient(var(--grid-lines) 1px, transparent 1px),
          linear-gradient(90deg, var(--grid-lines) 1px, transparent 1px)
        `,
        backgroundSize: `${GRID * initial.scale}px ${GRID * initial.scale}px`,
        backgroundPosition: `${initial.x}px ${initial.y}px`,
      }}
    />
  );
};

const TransformContainer = ({ groupId, children }: { groupId: string, children: React.ReactNode }) => {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // 零 React 重绘：平移缩放时直接操作 transform，跳过 Virtual DOM Diff
    return useViewportStore.subscribe(state => {
      const canvas = state.viewports[groupId] || DEFAULT_VIEWPORT;
      const el = containerRef.current;
      if (el) {
        // 使用 translate3d 触发 GPU 加速，确保 CSS 格式正确
        el.style.transform = `translate3d(${canvas.x}px, ${canvas.y}px, 0) scale(${canvas.scale})`;
      }
    });
  }, [groupId]);

  const initial = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
  return (
    <div
      ref={containerRef}
      style={{
        transform: `translate3d(${initial.x}px, ${initial.y}px, 0) scale(${initial.scale})`,
        transformOrigin: "0 0",
        willChange: "transform", // 提示浏览器开启图层优化
      }}
    >
      {children}
    </div>
  );
};

const EdgesLayer = ({
  groupId,
  visibleNodeIds,
  pinNodeIdIndex,
  getPinWorldPos,
  getCanvasLocalPoint,
  gesture,
  pendingConnection,
  contextMenu,
  activeGroupId,
  theme
}: any) => {
  const edgeCanvasRef = useRef<HTMLCanvasElement>(null);
  const rafRef = useRef<number | null>(null);

  // 绘制连接线的核心逻辑 (GPU 加速)
  const drawAllEdges = useCallback(() => {
    const canvasEl = edgeCanvasRef.current;
    if (!canvasEl) return;
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;

    const canvas = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;

    // 清除画布
    ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

    // 设置变换矩阵 (同步画布的平移和缩放)
    ctx.save();
    ctx.translate(canvas.x, canvas.y);
    ctx.scale(canvas.scale, canvas.scale);

    // 绘制已有连接
    const allNodes = useNodeStore.getState().nodes;
    const activeIds = useNodeStore.getState().activeNodeIds;

    activeIds.forEach(id => {
      const node = allNodes[id];
      if (!node) return;
      
      // 暂时移除 Edges 层的视口裁剪判断，确保所有连接线都能绘制
      // 因为 getPinWorldPos 内部已经处理了未渲染节点的 null 返回
      
      node.outputs.forEach((pin: any) => {
        pin.links.forEach((targetId: string) => {
          const start = getPinWorldPos(pin.id);
          const end = getPinWorldPos(targetId);
          if (!start || !end) return;

          drawEdge(
            ctx,
            start.x, start.y,
            end.x, end.y,
            pin.ui?.color ?? (theme[`${pin.type}Color` as keyof typeof theme] as string) ?? theme.connectionLines,
            2 / canvas.scale // 保持视觉粗细一致
          );
        });
      });
    });

    // 绘制当前正在拖拽的连接线
    const isInteracting = gesture?.type === "connect" || (pendingConnection && contextMenu?.visible);
    // 关键修复：只有当前 active 的 Canvas 才绘制交互线，防止其他分屏视口错误绘制
    const shouldDrawInteraction = isInteracting && (activeGroupId === groupId);

    if (shouldDrawInteraction) {
      const pin = gesture?.type === "connect" ? gesture.startPin : pendingConnection!;
      const start = getPinWorldPos(pin.id);
      if (!start) return;

      const end = gesture?.type === "connect"
        ? getCanvasLocalPoint(gesture.currentX, gesture.currentY)
        : getCanvasLocalPoint(contextMenu?.x || 0, contextMenu?.y || 0);

      drawEdge(
        ctx,
        start.x, start.y,
        end.x, end.y,
        pin.ui?.color ?? (theme[`${pin.type}Color` as keyof typeof theme] as string) ?? theme.connectionLines,
        2 / canvas.scale,
        pin.direction === "input"
      );
    }

    ctx.restore();
  }, [gesture, pendingConnection, contextMenu, getPinWorldPos, getCanvasLocalPoint, theme, groupId, activeGroupId, visibleNodeIds, pinNodeIdIndex]);

  const requestDraw = useCallback(() => {
    if (rafRef.current) return;
    rafRef.current = requestAnimationFrame(() => {
      drawAllEdges();
      rafRef.current = null;
    });
  }, [drawAllEdges]);

  // 监听 ViewportStore 和 NodeStore 的变化，触发重绘
  useEffect(() => {
    const unsubViewport = useViewportStore.subscribe(() => {
      requestDraw();
    });
    const unsubNodes = useNodeStore.subscribe(() => {
      requestDraw();
    });
    return () => {
      unsubViewport();
      unsubNodes();
    };
  }, [requestDraw]);

  // 同步画布尺寸并触发重绘
  useLayoutEffect(() => {
    const canvasEl = edgeCanvasRef.current;
    if (!canvasEl) return;

    // 确保从正确的父元素获取尺寸
    const rect = canvasEl.parentElement?.getBoundingClientRect();
    if (!rect || rect.width === 0 || rect.height === 0) return;

    const dpr = window.devicePixelRatio || 1;

    // 设置实际像素大小 (防止模糊)
    canvasEl.width = rect.width * dpr;
    canvasEl.height = rect.height * dpr;
    // 设置 CSS 大小
    canvasEl.style.width = `${rect.width}px`;
    canvasEl.style.height = `${rect.height}px`;

    const ctx = canvasEl.getContext("2d");
    if (ctx) {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0); // 使用 setTransform 替代 scale 避免累加
    }

    drawAllEdges();
  }, [drawAllEdges]);

  return <canvas ref={edgeCanvasRef} className="absolute inset-0 pointer-events-none" />;
};

export default function InfiniteCanvas() {
  const {
    setCanvas,
    setNodes,
    onCanvasPointerDown,
    onNodePointerDown,
    onPinPointerDown,
    selection,
    gesture,
    contextMenu,
    setContextMenu,
    variables,
    globalVariables,
    undo,
    redo,
    saveHistory,
    copy,
    paste,
    cut,
    deleteSelected,
    executeGraph,
    saveGraph,
    saveGraphAs,
    importGraph,
    addEvent,
    addFunction,
    addMacro,
    activeTabId,
    pendingConnection,
    setPendingConnection,
    tabs,
    setActiveTabId,
    closeTab,
    functions,
    macros,
    connectPins,
    splitEditorRight,
    groupId,
    activeGroupId, // Added activeGroupId
    selectedNodeIds
  } = useCanvas();
  const { theme } = useTheme();
  const { drag } = useDrag();

  const activeNodeIds = useNodeStore(state => state.activeNodeIds);
  const scale = useViewportStore(useCallback(state => state.viewports[groupId]?.scale || 1, [groupId]));

  // --- 视口裁剪 (Culling) 逻辑 ---
  const [visibleNodeIds, setVisibleNodes] = useState<Set<string>>(new Set());
  
  const updateVisibleNodes = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const canvas = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
    const allNodes = useNodeStore.getState().nodes;
    const activeIds = useNodeStore.getState().activeNodeIds;
    
    // 计算当前视口在世界坐标系中的范围 (增加 500px 缓冲)
    const padding = 500 / canvas.scale;
    const left = -canvas.x / canvas.scale - padding;
    const top = -canvas.y / canvas.scale - padding;
    const right = (rect.width - canvas.x) / canvas.scale + padding;
    const bottom = (rect.height - canvas.y) / canvas.scale + padding;

    const visible = new Set<string>();
    activeIds.forEach(id => {
      const node = allNodes[id];
      if (!node) return;
      // 简单的矩形相交判断 (假设节点最大宽高为 300x300)
      if (node.position.x + 300 > left && node.position.x < right &&
          node.position.y + 300 > top && node.position.y < bottom) {
        visible.add(id);
      }
    });
    setVisibleNodes(visible);
  }, [groupId]);

  // 当节点列表变化、缩放变化或平移结束时更新裁剪
  useEffect(() => {
    return useNodeStore.subscribe((state, prevState) => {
      if (state.activeNodeIds !== prevState.activeNodeIds) {
        updateVisibleNodes();
      }
    });
  }, [updateVisibleNodes]);

  useEffect(() => {
    updateVisibleNodes();
  }, [scale, activeNodeIds, updateVisibleNodes]);

  // 监听平移结束（通过 gesture 状态判断）
  useEffect(() => {
    if (!gesture) {
      updateVisibleNodes();
    }
  }, [gesture, updateVisibleNodes]);

  // 使用原生事件监听器以更好地控制 preventDefault 和 stopPropagation
  useEffect(() => {
    const canvasEl = ref.current;
    if (!canvasEl) return;

    const handleWheel = (e: WheelEvent) => {
      // 检查点击来源：如果是 UI 组件，则完全忽略
      const target = e.target as HTMLElement;
      if (
        target.closest(".menubar-container") ||
        target.closest(".sidebar-container") ||
        target.closest(".menu-container") ||
        target.closest(".hud-container")
      ) {
        return;
      }

      // 如果鼠标不在画布范围内，也不处理
      const rect = canvasEl.getBoundingClientRect();
      if (
        e.clientX < rect.left ||
        e.clientX > rect.right ||
        e.clientY < rect.top ||
        e.clientY > rect.bottom
      ) {
        return;
      }

      // 阻止浏览器默认的缩放或滚动行为
      e.preventDefault();

      // 直接在此处执行缩放逻辑，使用 Store 保证性能
      const factor = 0.001;
      const currentCanvas = useViewportStore.getState().viewports[groupId] || { x: 0, y: 0, scale: 1 };
      const nextScale = Math.min(Math.max(currentCanvas.scale * (1 - e.deltaY * factor), 0.2), 4);

      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;

      const worldX = (mouseX - currentCanvas.x) / currentCanvas.scale;
      const worldY = (mouseY - currentCanvas.y) / currentCanvas.scale;

      setCanvas({
        scale: nextScale,
        x: mouseX - worldX * nextScale,
        y: mouseY - worldY * nextScale,
      });
    };

    window.addEventListener("wheel", handleWheel, { passive: false, capture: true });
    return () => window.removeEventListener("wheel", handleWheel, { capture: true });
  }, [setCanvas, groupId]); // Added groupId dependency

  const selectedNodeIdsSet = useMemo(() => {
    return new Set(selectedNodeIds);
  }, [selectedNodeIds]);

  const selectedNodeIdsRef = useRef(selectedNodeIdsSet);
  useEffect(() => {
    selectedNodeIdsRef.current = selectedNodeIdsSet;
  }, [selectedNodeIdsSet]);

  const [variableDropMenu, setVariableDropMenu] = useState<{
    x: number;
    y: number;
    worldX: number;
    worldY: number;
    variableId: string;
    variableName: string;
    variableType: string;
  } | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  const prevDragRef = useRef(drag);

  const pinIndex = useMemo(() => {
    const map = new Map<string, Pin>();
    const allNodes = useNodeStore.getState().nodes;
    activeNodeIds.forEach(id => {
      const node = allNodes[id];
      if (!node) return;
      node.inputs.forEach((pin: Pin) => map.set(pin.id, pin));
      node.outputs.forEach((pin: Pin) => map.set(pin.id, pin));
    });
    return map;
  }, [activeTabId, activeNodeIds]); // Recompute when tab switch or node list changes

  const pinNodeIdIndex = useMemo(() => {
    const map = new Map<string, string>();
    const allNodes = useNodeStore.getState().nodes;
    activeNodeIds.forEach(id => {
      const node = allNodes[id];
      if (!node) return;
      node.inputs.forEach((pin: Pin) => map.set(pin.id, node.id));
      node.outputs.forEach((pin: Pin) => map.set(pin.id, node.id));
    });
    return map;
  }, [activeTabId, activeNodeIds]);

  const pinIndexRef = useRef(pinIndex);
  useEffect(() => {
    pinIndexRef.current = pinIndex;
  }, [pinIndex]);



  const [pinOffsets, setPinOffsets] = useState<Record<string, { x: number; y: number }>>({});

  // 测量 Pin 相对于节点的偏移量 (仅在节点或 Pin 数量变化时运行)
  // 修复：使用 scoped querySelector 替代 getElementById，确保在分屏时测量的是当前视口内的节点
  useLayoutEffect(() => {
    const root = ref.current;
    if (!root) return;
    const nextOffsets: Record<string, { x: number; y: number }> = {};
    const allNodes = useNodeStore.getState().nodes;
    const activeIds = useNodeStore.getState().activeNodeIds;

    activeIds.forEach(id => {
      const node = allNodes[id];
      if (!node) return;
      // 关键修复：只在当前 Canvas 容器内查找节点
      const nodeEl = root.querySelector(`[data-node-id="${node.id}"]`);
      if (!nodeEl) return;

      const nodeRect = nodeEl.getBoundingClientRect();
      const pins = nodeEl.querySelectorAll<HTMLElement>("[data-pin-id]");

      pins.forEach(pinEl => {
        const pinId = pinEl.dataset.pinId;
        if (!pinId) return;

        // Find the actual visual center (the circle)
        const circleEl = pinEl.querySelector(".pin-circle");
        const targetEl = circleEl || pinEl;
        const rect = targetEl.getBoundingClientRect();

        nextOffsets[pinId] = {
          x: (rect.left + rect.width / 2 - nodeRect.left) / scale,
          y: (rect.top + rect.height / 2 - nodeRect.top) / scale,
        };
      });
    });

    setPinOffsets(prev => {
      // Check if anything actually changed
      const currentKeys = Object.keys(nextOffsets);
      const prevKeys = Object.keys(prev);

      if (currentKeys.length === prevKeys.length) {
        const isSame = currentKeys.every(k =>
          prev[k] &&
          Math.abs(prev[k].x - nextOffsets[k].x) < 0.1 &&
          Math.abs(prev[k].y - nextOffsets[k].y) < 0.1
        );
        if (isSame) return prev;
      }
      return nextOffsets;
    });
  }, [activeTabId, scale, visibleNodeIds, activeNodeIds]); // Re-measure on tab switch, node array change, or scale change

  // 获取 Pin 的 world 坐标
  const getPinWorldPos = useCallback((pinId: string) => {
    const nodeId = pinNodeIdIndex.get(pinId);
    const node = useNodeStore.getState().nodes[nodeId || ""];
    const offset = pinOffsets[pinId];
    if (!node || !offset) return null;
    return {
      x: node.position.x + offset.x,
      y: node.position.y + offset.y
    };
  }, [pinNodeIdIndex, pinOffsets]);

  const getCanvasLocalPoint = useCallback((clientX: number, clientY: number) => {
    const root = ref.current;
    if (!root) return { x: 0, y: 0 };
    const rect = root.getBoundingClientRect();
    const currentCanvas = useViewportStore.getState().viewports[groupId] || { x: 0, y: 0, scale: 1 };
    return {
      x: (clientX - rect.left - currentCanvas.x) / currentCanvas.scale,
      y: (clientY - rect.top - currentCanvas.y) / currentCanvas.scale
    };
  }, [groupId]);

  // 获取 Pin 的世界坐标 (Node位置 + 偏移)

  const handleNodePaletteSelect = (tpl: { type: string }) => {
    if (!contextMenu || !ref.current) return;

    // Check if this is an internal node type that should only exist once
    const internalNodeTypes = ['event_on_run', 'function_entry', 'function_return', 'macro_inputs', 'macro_outputs'];
    if (internalNodeTypes.includes(tpl.type)) {
      // Check if this internal node already exists
      const allNodes = Object.values(useNodeStore.getState().nodes);
      const existingNode = allNodes.find(n => n.type === tpl.type && n.isInternal);
      if (existingNode) {
        // Move canvas to center on the existing node
        const rect = ref.current.getBoundingClientRect();
        const centerX = rect.width / 2;
        const centerY = rect.height / 2;

        const currentCanvas = useViewportStore.getState().viewports[groupId] || { x: 0, y: 0, scale: 1 };
        setCanvas({
          ...currentCanvas,
          x: centerX - existingNode.position.x * currentCanvas.scale,
          y: centerY - existingNode.position.y * currentCanvas.scale
        });

        setContextMenu(null);
        setPendingConnection(null);
        return;
      }
    }

    const rect = ref.current.getBoundingClientRect();
    const currentCanvas = useViewportStore.getState().viewports[groupId] || { x: 0, y: 0, scale: 1 };
    const x = (contextMenu.x - rect.left - currentCanvas.x) / currentCanvas.scale;
    const y = (contextMenu.y - rect.top - currentCanvas.y) / currentCanvas.scale;

    const newNode = createNodeFromTemplate({ x, y }, currentCanvas.scale, tpl.type);
    if (newNode) {
      saveHistory();

      const currentNodes = Object.values(useNodeStore.getState().nodes);
      const newNodes = [...currentNodes, newNode];
      setNodes(newNodes);

      // 如果有待处理的连接，尝试自动连接
      if (pendingConnection) {
        const isInput = pendingConnection.direction === "input";
        const targetDirection = isInput ? "outputs" : "inputs";

        // 寻找新节点中第一个符合类型的引脚
        const pins = targetDirection === "inputs" ? newNode.inputs : newNode.outputs;
        const compatiblePin = pins.find(p => p.type === pendingConnection.type);

        if (compatiblePin) {
          // 延迟一帧调用 connectPins 确保 nodes 已更新
          setTimeout(() => {
            connectPins(pendingConnection.id, compatiblePin.id);
          }, 0);
        }
      }
    }
    setContextMenu(null);
    setPendingConnection(null);
  };

  // 1. 自动隐藏菜单逻辑
  useEffect(() => {
    const handleClickOutside = (e: PointerEvent) => {
      const target = e.target as HTMLElement;
      // 检查点击是否在菜单容器之外
      const isInsideMenu = target.closest(".menu-container");
      if (!isInsideMenu) {
        if (contextMenu?.visible) {
          setContextMenu(null);
          setPendingConnection(null);
        }
        if (variableDropMenu) setVariableDropMenu(null);
      }
    };

    window.addEventListener("pointerdown", handleClickOutside, true); // 使用捕获阶段
    return () => window.removeEventListener("pointerdown", handleClickOutside, true);
  }, [contextMenu, variableDropMenu, setContextMenu, setPendingConnection]);

  // Selection and node intersection is now managed by the CanvasProvider's gesture system

  // Removed local handleNodePointerDown and handleNodeDrag
  // using provider ones instead


  const lastMousePosRef = useRef({ x: 0, y: 0 });

  useEffect(() => {
    const handlePointerMove = (e: PointerEvent) => {
      lastMousePosRef.current = { x: e.clientX, y: e.clientY };
    };
    window.addEventListener("pointermove", handlePointerMove, { capture: true });
    return () => window.removeEventListener("pointermove", handlePointerMove, { capture: true });
  }, []);

  // 2. 多节点拖拽回调
  // handleNodeDrag removed

  // 3. 动态添加输入
  const handleNodeAddInput = useCallback((id: string) => {
    saveHistory();
    const currentNodes = Object.values(useNodeStore.getState().nodes);
    setNodes(
      currentNodes.map((node) => {
        if (node.id === id) {
          const newNode = node.clone();
          const newIndex = newNode.inputs.length;
          newNode.addInput({
            id: `${id}-input-${newIndex}-${Date.now()}`,
            nodeId: id,
            name: String.fromCharCode(65 + newIndex),
            type: "int", // 修改这里：PinType 中已改为 int
            direction: "input",
            links: [],
          });
          return newNode;
        }
        return node;
      })
    );
  }, [saveHistory, setNodes]);

  const handlePinClick = useCallback((pinId: string, _direction: "input" | "output") => {
    console.log(`Pin clicked: ${pinId}`);
  }, []);

  useEffect(() => {
    // 从「有拖拽」→「无拖拽」 = drop
    if (prevDragRef.current && !drag) {
      const last = prevDragRef.current;

      if (last.type === "node-template") {
        handleDropTemplate(last, {
          altKey: (window as any)._lastAltKey || false,
          ctrlKey: (window as any)._lastCtrlKey || false,
        } as any);
      }
    }

    prevDragRef.current = drag;
  }, [drag, variables]); // 增加 variables 依赖，确保 handleDropTemplate 使用最新状态

  // 全局记录修饰键状态
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      (window as any)._lastAltKey = e.altKey;
      (window as any)._lastCtrlKey = e.ctrlKey;

      // 防止在输入框中触发破坏性快捷键（如 Delete），
      // 但允许全局控制快捷键（如 Ctrl+S, Ctrl+Z, Ctrl+Tab）
      const isInput =
        document.activeElement?.tagName === "INPUT" ||
        document.activeElement?.tagName === "TEXTAREA" ||
        (document.activeElement as HTMLElement)?.isContentEditable;

      const isControlKey = e.ctrlKey || e.metaKey;

      if (isInput) {
        // 在输入框中，仅允许特定的全局快捷键通过
        const allowedInInput =
          (isControlKey && ["s", "z", "y", "n", "o", "w"].includes(e.key.toLowerCase())) ||
          (isControlKey && e.key === "Tab");

        if (!allowedInInput) return;
      }

      if (e.key === "Delete" || e.key === "Backspace") {
        deleteSelected();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
        if (e.shiftKey) {
          redo();
        } else {
          undo();
        }
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") {
        redo();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") {
        copy();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "x") {
        cut();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "v") {
        paste(getCanvasLocalPoint(lastMousePosRef.current.x, lastMousePosRef.current.y));
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (e.shiftKey) {
          saveGraphAs();
        } else {
          saveGraph();
        }
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "o") {
        e.preventDefault();
        importGraph();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "n") {
        e.preventDefault();
        addEvent("New Graph");
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "w") {
        e.preventDefault();
        if (activeTabId) closeTab(activeTabId);
      } else if ((e.ctrlKey || e.metaKey) && e.key === "Tab") {
        e.preventDefault();
        if (tabs.length > 1) {
          const currentIndex = tabs.findIndex(t => t.id === activeTabId);
          let nextIndex;
          if (e.shiftKey) {
            nextIndex = (currentIndex - 1 + tabs.length) % tabs.length;
          } else {
            nextIndex = (currentIndex + 1) % tabs.length;
          }
          setActiveTabId(tabs[nextIndex].id);
        }
      } else if ((e.ctrlKey || e.metaKey) && e.key === "\\") {
        e.preventDefault();
        splitEditorRight(groupId);
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      (window as any)._lastAltKey = e.altKey;
      (window as any)._lastCtrlKey = e.ctrlKey;
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [deleteSelected, copy, paste, cut, undo, redo, getCanvasLocalPoint, saveGraph, saveGraphAs, importGraph, addEvent, closeTab, activeTabId, tabs, setActiveTabId, splitEditorRight, groupId]);

  /* ===== Data Drag ===== */
  function handleDropTemplate(dragState: any, event: MouseEvent | PointerEvent) {
    const el = ref.current;
    if (!el) return;

    const rect = el.getBoundingClientRect();

    // 边界检查：确保释放点在画布范围内
    const isInside =
      dragState.x >= rect.left &&
      dragState.x <= rect.right &&
      dragState.y >= rect.top &&
      dragState.y <= rect.bottom;

    if (!isInside) return;

    const screenX = dragState.x - rect.left;
    const screenY = dragState.y - rect.top;

    const currentCanvas = useViewportStore.getState().viewports[groupId] || { x: 0, y: 0, scale: 1 };
    const x = (screenX - currentCanvas.x) / currentCanvas.scale;
    const y = (screenY - currentCanvas.y) / currentCanvas.scale;

    // 检查是否落在了 pin 上 (需求 3: 拖动变量到赋值框)
    const elements = document.elementsFromPoint(dragState.x, dragState.y);
    const pinEl = elements.find(e => e.closest("[data-pin-id]"))?.closest("[data-pin-id]");
    const targetPinId = pinEl?.getAttribute("data-pin-id");

    // 如果是变量
    if (dragState.template.category === "Variable") {
      // 安全检查：确保变量依然存在（检查局部和全局）
      if (!variables[dragState.template.variableId] && !globalVariables[dragState.template.variableId]) {
        console.warn("Variable no longer exists. Aborting drop.");
        return;
      }

      let spawnType: "get_variable" | "set_variable" | null = null;

      if (event.altKey) spawnType = "set_variable";
      else if (event.ctrlKey) spawnType = "get_variable";

      if (spawnType) {
        saveHistory();
        const newNode = createNodeFromTemplate({ x, y }, currentCanvas.scale, spawnType);
        if (newNode) {
          const currentNodes = Object.values(useNodeStore.getState().nodes);
          setNodes([...currentNodes, newNode]);

          if (targetPinId && spawnType === "get_variable") {
            const outputPin = newNode.outputs[0];
            if (outputPin) {
              setTimeout(() => {
                connectPins(outputPin.id, targetPinId);
              }, 0);
            }
          }
        }
        return;
      }

      if (targetPinId) {
        const newNode = createNodeFromTemplate({ x, y }, currentCanvas.scale, "get_variable");
        if (newNode) {
          const currentNodes = Object.values(useNodeStore.getState().nodes);
          setNodes([...currentNodes, newNode]);
          const outputPin = newNode.outputs[0];
          if (outputPin) {
            setTimeout(() => {
              connectPins(outputPin.id, targetPinId);
            }, 0);
          }
          return;
        }
      }

      setVariableDropMenu({
        x: dragState.x,
        y: dragState.y,
        worldX: x,
        worldY: y,
        variableId: dragState.template.variableId,
        variableName: dragState.template.variableName,
        variableType: dragState.template.variableType,
      });
      return;
    } else if (dragState.template.type === "call_function" || dragState.template.type === "call_macro") {
      saveHistory();
      const type = dragState.template.type;
      const subId = dragState.template.subGraphId;
      const subName = dragState.template.subName;
      const subData = (type === 'call_function') ? functions[subId] : macros[subId];
      if (!subData) return;
      const node = createInternalNode(
        `node-${crypto.randomUUID()}`,
        type,
        subName,
        type === 'call_function' ? "Functions" : "Macros",
        { x, y },
        [{ id: `exec-in-${Date.now()}`, nodeId: "", name: "In", type: "exec", direction: "input", links: [] },
        ...(subData.inputs || []).map(p => ({ id: `in-${p.id}-${Date.now()}`, nodeId: "", name: p.name, type: p.type as any, direction: "input", links: [] as string[] }))],
        [{ id: `exec-out-${Date.now()}`, nodeId: "", name: "Out", type: "exec", direction: "output", links: [] },
        ...(subData.outputs || []).map(p => ({ id: `out-${p.id}-${Date.now()}`, nodeId: "", name: p.name, type: p.type as any, direction: "output", links: [] as string[] }))],
        false
      );
      node.subGraphId = subId;
      const currentNodes = Object.values(useNodeStore.getState().nodes);
      setNodes([...currentNodes, node]);
      return;
    }

    const newNode = createNodeFromTemplate(
      { x, y },
      currentCanvas.scale,
      dragState.template.type
    );
    if (newNode) {
      saveHistory();
      const currentNodes = Object.values(useNodeStore.getState().nodes);
      setNodes([...currentNodes, newNode]);
    }
  }

  const activePin = useMemo(() => {
    if (gesture?.type === "connect") return gesture.startPin;
    if (pendingConnection && contextMenu?.visible) return pendingConnection;
    return null;
  }, [gesture, pendingConnection, contextMenu]);

  if (activeTabId === null) {
    return (
      <div className="relative w-full h-full flex flex-col items-center justify-center bg-[var(--workbench-bg)] select-none overflow-hidden">
        {/* Simplified Logo */}
        <div className="mb-8 opacity-20 group">
          <svg className="w-32 h-32 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1">
            <path strokeLinecap="round" strokeLinejoin="round" d="M11 3.055A9.001 9.001 0 1020.945 13H11V3.055z" />
            <path strokeLinecap="round" strokeLinejoin="round" d="M20.488 9H15V3.512A9.025 9.025 0 0120.488 9z" />
          </svg>
        </div>
        {/* Shortcut Hints */}
        <div className="flex flex-col gap-4 items-start text-gray-500 text-sm font-medium">
          <div className="flex items-center gap-12 justify-between w-full min-w-[340px] hover:bg-white/5 p-2 rounded transition-colors group cursor-pointer" onClick={() => addEvent("New Event")}>
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full bg-red-500" />
              <span>新建 Event Graph</span>
            </div>
            <span className="text-[10px] text-gray-600 italic">Core logic</span>
          </div>
          <div className="flex items-center gap-12 justify-between w-full hover:bg-white/5 p-2 rounded transition-colors group cursor-pointer" onClick={() => addFunction("New Function")}>
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full bg-blue-500" />
              <span>新建 Function</span>
            </div>
            <span className="text-[10px] text-gray-600 italic">Reusable routine</span>
          </div>
          <div className="flex items-center gap-12 justify-between w-full hover:bg-white/5 p-2 rounded transition-colors group cursor-pointer" onClick={() => addMacro("New Macro")}>
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full bg-purple-500" />
              <span>新建 Macro</span>
            </div>
            <span className="text-[10px] text-gray-600 italic">Node pattern</span>
          </div>
          <div className="flex items-center gap-12 justify-between w-full hover:bg-white/5 p-2 rounded transition-colors group cursor-pointer" onClick={() => importGraph()}>
            <span>打开文件</span>
            <span className="flex gap-1">
              <kbd className="px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-[10px] text-gray-400">Ctrl</kbd>
              <kbd className="px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-[10px] text-gray-400">O</kbd>
            </span>
          </div>
          <div className="flex items-center gap-12 justify-between w-full hover:bg-white/5 p-2 rounded transition-colors group cursor-not-allowed opacity-50">
            <span>显示所有命令</span>
            <span className="flex gap-1">
              <kbd className="px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-[10px] text-gray-400">Ctrl</kbd>
              <kbd className="px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-[10px] text-gray-400">Shift</kbd>
              <kbd className="px-1.5 py-0.5 bg-gray-800 border border-gray-700 rounded text-[10px] text-gray-400">P</kbd>
            </span>
          </div>
        </div>
        {/* Subtle grid background for the empty state too, but very faint */}
        <div className="absolute inset-0 opacity-[0.03] pointer-events-none"
          style={{
            backgroundImage: `linear-gradient(#fff 1px, transparent 1px), linear-gradient(90deg, #fff 1px, transparent 1px)`,
            backgroundSize: '40px 40px'
          }}
        />
      </div>
    );
  }


  return (
    <div
      ref={ref}
      className="relative w-full h-full overflow-hidden bg-[var(--workbench-bg)] select-none"
    >
      {/* ================= Grid ================= */}
      <ViewportGrid groupId={groupId} />

      {/* ================= World ================= */}
      <div
        className="absolute inset-0"
        onPointerDown={onCanvasPointerDown}
        onContextMenu={(e) => e.preventDefault()}
      >
        {/* GPU 加速的连接线层 */}
        <EdgesLayer
          groupId={groupId}
          visibleNodeIds={visibleNodeIds}
          pinNodeIdIndex={pinNodeIdIndex}
          getPinWorldPos={getPinWorldPos}
          getCanvasLocalPoint={getCanvasLocalPoint}
          gesture={gesture}
          pendingConnection={pendingConnection}
          contextMenu={contextMenu}
          activeGroupId={activeGroupId}
          theme={theme}
        />

        <TransformContainer groupId={groupId}>
          {activeNodeIds.filter(id => visibleNodeIds.has(id)).map((nodeId) => (
            <Node
              key={nodeId}
              id={nodeId}
              scale={scale}
              selected={selectedNodeIdsSet.has(nodeId)}
              activePinId={activePin?.id}
              onPointerDown={(id, e) => onNodePointerDown(id, e)}
              onAddInput={handleNodeAddInput}
              onPinClick={handlePinClick}
              onPinPointerDown={(e, p) => onPinPointerDown(p.id, e)}
            />
          ))}
        </TransformContainer>
      </div>
      <HUD />

      {/* ================= FAB (Floating Action Button) for Execution ================= */}
      {tabs.find(t => t.id === activeTabId)?.type === "event" && (
        <div className="absolute top-4 right-4 z-40">
          <button
            onClick={() => executeGraph()}
            className="flex items-center gap-2 px-6 py-2.5 bg-green-600 hover:bg-green-500 text-white rounded-full shadow-lg transition-all active:scale-95 text-xs font-bold ring-4 ring-black/20"
          >
            <VscRunAll size={18} />
            <span>执行</span>
          </button>
        </div>
      )}

      {/* ================= Selection Box ================= */}
      {selection && ref.current && (
        <div
          className="absolute border border-[var(--accent-color)] bg-[var(--selection-region)] pointer-events-none z-50"
          style={{
            left:
              Math.min(selection.startX, selection.currentX) -
              ref.current.getBoundingClientRect().left,
            top:
              Math.min(selection.startY, selection.currentY) -
              ref.current.getBoundingClientRect().top,
            width: Math.abs(selection.startX - selection.currentX),
            height: Math.abs(selection.startY - selection.currentY),
          }}
        />
      )}

      {/* ================= Node Palette ================= */}
      {activeGroupId === groupId && contextMenu?.visible && (
        <NodePalette
          x={contextMenu.x}
          y={contextMenu.y}
          onSelect={handleNodePaletteSelect}
          filterPin={pendingConnection}
        />
      )}

      {/* ================= Variable Drop Menu ================= */}
      {activeGroupId === groupId && variableDropMenu && (
        <div
          className="fixed z-50 bg-gray-800 text-white rounded shadow-lg overflow-hidden border border-gray-700 py-1 menu-container"
          style={{ left: variableDropMenu.x, top: variableDropMenu.y }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          <div
            className="px-4 py-2 hover:bg-gray-600 cursor-pointer text-sm font-bold flex items-center gap-2"
            onClick={() => {
              if (!variables[variableDropMenu.variableId] && !globalVariables[variableDropMenu.variableId]) {
                console.warn("Variable no longer exists.");
                setVariableDropMenu(null);
                return;
              }
              saveHistory();
              const newNode = createNodeFromTemplate(
                { x: variableDropMenu.worldX, y: variableDropMenu.worldY },
                scale,
                "get_variable",
                {
                  title: `Get ${variableDropMenu.variableName}`,
                  variableId: variableDropMenu.variableId,
                  variableType: variableDropMenu.variableType // 传入初始变量类型
                }
              );
              if (newNode) {
                const currentNodes = Object.values(useNodeStore.getState().nodes);
                setNodes([...currentNodes, newNode]);
              }
              setVariableDropMenu(null);
            }}
          >
            <div className="w-2 h-2 rounded-full bg-blue-400" />
            Get {variableDropMenu.variableName}
          </div>
          <div
            className="px-4 py-2 hover:bg-gray-600 cursor-pointer text-sm font-bold flex items-center gap-2 border-t border-gray-700"
            onClick={() => {
              if (!variables[variableDropMenu.variableId] && !globalVariables[variableDropMenu.variableId]) {
                console.warn("Variable no longer exists.");
                setVariableDropMenu(null);
                return;
              }
              saveHistory();
              const newNode = createNodeFromTemplate(
                { x: variableDropMenu.worldX, y: variableDropMenu.worldY },
                scale,
                "set_variable",
                {
                  title: `Set ${variableDropMenu.variableName}`,
                  variableId: variableDropMenu.variableId,
                  variableType: variableDropMenu.variableType // 传入初始变量类型
                }
              );
              if (newNode) {
                const currentNodes = Object.values(useNodeStore.getState().nodes);
                setNodes([...currentNodes, newNode]);
              }
              setVariableDropMenu(null);
            }}
          >
            <div className="w-2 h-2 rounded-full bg-orange-400" />
            Set {variableDropMenu.variableName}
          </div>
        </div>
      )}
    </div>
  );
}
