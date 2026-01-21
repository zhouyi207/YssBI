import { useRef, useState, useEffect, useCallback, useMemo, useLayoutEffect } from "react";
import { Node } from "../Nodes/Node";
import { BaseNode, Pin } from "../Types/nodes";
import { useDrag } from "../Context/DragContext";
import { useCanvas } from "../Context/CanvasContext";
import { createNodeFromTemplate } from "../Utils/nodeUtils";
import { createInternalNode } from "../Utils/internalNodes";
import HUD from "./HUD";
import NodePalette from "../Nodes/NodePalette";
import { drawEdge } from "../Edges/Edge";

/* ================= Canvas ================= */

export default function InfiniteCanvas() {
  const {
    canvas,
    setCanvas,
    nodes,
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
    connectPins
  } = useCanvas();


  const { drag } = useDrag();

  // 优化：使用 Ref 记录最新的 canvas 状态，避免 useEffect 频繁卸载/挂载监听器
  const canvasRef = useRef(canvas);
  useEffect(() => {
    canvasRef.current = canvas;
  }, [canvas]);

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

      // 直接在此处执行缩放逻辑，使用 Ref 保证性能和稳定性
      const factor = 0.001;
      const currentCanvas = canvasRef.current;
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
  }, [setCanvas]); // 仅依赖 setCanvas

  const edgeCanvasRef = useRef<HTMLCanvasElement>(null);

  const selectedNodeIds = useMemo(() => {
    const set = new Set<string>();
    nodes.forEach(n => {
      if (n.selected) set.add(n.id);
    });
    return set;
  }, [nodes]);

  const selectedNodeIdsRef = useRef(selectedNodeIds);
  useEffect(() => {
    selectedNodeIdsRef.current = selectedNodeIds;
  }, [selectedNodeIds]);

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
    nodes.forEach((node) => {
      node.inputs.forEach((pin: Pin) => map.set(pin.id, pin));
      node.outputs.forEach((pin: Pin) => map.set(pin.id, pin));
    });
    return map;
  }, [nodes]);

  const pinNodeIdIndex = useMemo(() => {
    const map = new Map<string, string>();
    nodes.forEach((node) => {
      node.inputs.forEach((pin: Pin) => map.set(pin.id, node.id));
      node.outputs.forEach((pin: Pin) => map.set(pin.id, node.id));
    });
    return map;
  }, [nodes]);

  const pinIndexRef = useRef(pinIndex);
  useEffect(() => {
    pinIndexRef.current = pinIndex;
  }, [pinIndex]);



  const [pinOffsets, setPinOffsets] = useState<Record<string, { x: number; y: number }>>({});

  // 测量 Pin 相对于节点的偏移量 (仅在节点或 Pin 数量变化时运行)
  useLayoutEffect(() => {
    const root = ref.current;
    if (!root) return;
    const nextOffsets: Record<string, { x: number; y: number }> = {};

    nodes.forEach(node => {
      const nodeEl = root.querySelector(`[id="${node.id}"]`);
      if (!nodeEl) return;
      const nodeRect = nodeEl.getBoundingClientRect();

      const pins = nodeEl.querySelectorAll<HTMLElement>("[data-pin-id]");
      pins.forEach(pinEl => {
        const pinId = pinEl.dataset.pinId;
        if (!pinId) return;
        const circleEl = pinEl.querySelector(".pin-circle");
        const targetEl = circleEl || pinEl;
        const rect = targetEl.getBoundingClientRect();

        // 计算相对于节点左上角的偏移，并还原缩放
        nextOffsets[pinId] = {
          x: (rect.left + rect.width / 2 - nodeRect.left) / canvas.scale,
          y: (rect.top + rect.height / 2 - nodeRect.top) / canvas.scale,
        };
      });
    });

    setPinOffsets(prev => {
      // 简单深度比较，避免无谓更新
      if (Object.keys(prev).length === Object.keys(nextOffsets).length) {
        let same = true;
        for (const key in nextOffsets) {
          if (!prev[key] || prev[key].x !== nextOffsets[key].x || prev[key].y !== nextOffsets[key].y) {
            same = false;
            break;
          }
        }
        if (same) return prev;
      }
      return nextOffsets;
    });
  }, [activeTabId, nodes.length, nodes.map(n => n.id).join(",")]); // 切换 Tab 或节点变化时必须重新测量

  const nodeMap = useMemo(() => {
    const map = new Map<string, BaseNode>();
    nodes.forEach(n => map.set(n.id, n));
    return map;
  }, [nodes]);

  // 获取 Pin 的世界坐标 (Node位置 + 偏移)
  const getPinWorldPos = useCallback((pinId: string) => {
    const nodeId = pinNodeIdIndex.get(pinId);
    const node = nodeMap.get(nodeId || "");
    const offset = pinOffsets[pinId];
    if (!node || !offset) return null;
    return {
      x: node.position.x + offset.x,
      y: node.position.y + offset.y
    };
  }, [nodeMap, pinNodeIdIndex, pinOffsets]);

  const getCanvasLocalPoint = useCallback((clientX: number, clientY: number) => {
    const root = ref.current;
    if (!root) return { x: 0, y: 0 };
    const rect = root.getBoundingClientRect();
    // 转换为 Canvas World 坐标
    return {
      x: (clientX - rect.left - canvas.x) / canvas.scale,
      y: (clientY - rect.top - canvas.y) / canvas.scale
    };
  }, [canvas.x, canvas.y, canvas.scale]);

  // 绘制连接线的核心逻辑 (GPU 加速)
  const drawAllEdges = useCallback(() => {
    const canvasEl = edgeCanvasRef.current;
    if (!canvasEl) return;
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;

    // 清除画布
    ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

    // 设置变换矩阵 (同步画布的平移和缩放)
    ctx.save();
    ctx.translate(canvas.x, canvas.y);
    ctx.scale(canvas.scale, canvas.scale);

    // 绘制已有连接
    nodes.forEach((node) => {
      node.outputs.forEach((pin: Pin) => {
        pin.links.forEach((targetId: string) => {
          const start = getPinWorldPos(pin.id);
          const end = getPinWorldPos(targetId);
          if (!start || !end) return;

          drawEdge(
            ctx,
            start.x, start.y,
            end.x, end.y,
            pin.ui?.color ?? (pin.type === "exec" ? "#ffffff" : "#3b82f6"),
            2 / canvas.scale // 保持视觉粗细一致
          );
        });
      });
    });

    // 绘制当前正在拖拽的连接线
    if (gesture?.type === "connect" || (pendingConnection && contextMenu?.visible)) {
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
        pin.ui?.color ?? (pin.type === "exec" ? "#ffffff" : "#3b82f6"),
        2 / canvas.scale,
        pin.direction === "input"
      );
    }

    ctx.restore();
  }, [nodes, canvas, gesture, pendingConnection, contextMenu, pinOffsets, getPinWorldPos, getCanvasLocalPoint]);

  // 同步画布尺寸并触发重绘
  useLayoutEffect(() => {
    const canvasEl = edgeCanvasRef.current;
    if (!canvasEl || !ref.current) return;

    const rect = ref.current.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;

    // 设置实际像素大小 (防止模糊)
    canvasEl.width = rect.width * dpr;
    canvasEl.height = rect.height * dpr;
    // 设置 CSS 大小
    canvasEl.style.width = `${rect.width}px`;
    canvasEl.style.height = `${rect.height}px`;

    const ctx = canvasEl.getContext("2d");
    if (ctx) ctx.scale(dpr, dpr);

    drawAllEdges();
  }, [drawAllEdges]);

  const handleNodePaletteSelect = (tpl: { type: string }) => {
    if (!contextMenu || !ref.current) return;

    // Check if this is an internal node type that should only exist once
    const internalNodeTypes = ['event_on_run', 'function_entry', 'function_return', 'macro_inputs', 'macro_outputs'];
    if (internalNodeTypes.includes(tpl.type)) {
      // Check if this internal node already exists
      const existingNode = nodes.find(n => n.type === tpl.type && n.isInternal);
      if (existingNode) {
        // Move canvas to center on the existing node
        const rect = ref.current.getBoundingClientRect();
        const centerX = rect.width / 2;
        const centerY = rect.height / 2;

        setCanvas({
          ...canvas,
          x: centerX - existingNode.position.x * canvas.scale,
          y: centerY - existingNode.position.y * canvas.scale
        });

        setContextMenu(null);
        setPendingConnection(null);
        return;
      }
    }

    const rect = ref.current.getBoundingClientRect();
    const x = (contextMenu.x - rect.left - canvas.x) / canvas.scale;
    const y = (contextMenu.y - rect.top - canvas.y) / canvas.scale;

    const newNode = createNodeFromTemplate({ x, y }, canvas.scale, tpl.type);
    if (newNode) {
      saveHistory();

      const newNodes = [...nodes, newNode];
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
    setNodes((prev) =>
      prev.map((node) => {
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

      // 防止在输入框中触发快捷键
      const isInput =
        document.activeElement?.tagName === "INPUT" ||
        document.activeElement?.tagName === "TEXTAREA" ||
        (document.activeElement as HTMLElement)?.isContentEditable;

      if (isInput) return;

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
  }, [deleteSelected, copy, paste, cut, undo, redo, getCanvasLocalPoint, saveGraph, saveGraphAs, importGraph, addEvent, closeTab, activeTabId, tabs, setActiveTabId]);

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

    const x = (screenX - canvas.x) / canvas.scale;
    const y = (screenY - canvas.y) / canvas.scale;

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
        const newNode = createNodeFromTemplate({ x, y }, canvas.scale, spawnType);
        if (newNode) {
          setNodes((prev) => [...prev, newNode]);

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
        const newNode = createNodeFromTemplate({ x, y }, canvas.scale, "get_variable");
        if (newNode) {
          setNodes(prev => [...prev, newNode]);
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
      setNodes((prev) => [...prev, node]);
      return;
    }

    const newNode = createNodeFromTemplate(
      { x, y },
      canvas.scale,
      dragState.template.type
    );
    if (newNode) {
      saveHistory();
      setNodes((prev) => [...prev, newNode]);
    }
  }

  const GRID = 40;

  const activePin = useMemo(() => {
    if (gesture?.type === "connect") return gesture.startPin;
    if (pendingConnection && contextMenu?.visible) return pendingConnection;
    return null;
  }, [gesture, pendingConnection, contextMenu]);

  if (activeTabId === null) {
    return (
      <div className="relative w-full h-full flex flex-col items-center justify-center bg-[#1e1e1e] select-none overflow-hidden">
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
      className="relative w-full h-full overflow-hidden bg-gray-900 select-none"
    >
      {/* ================= Grid ================= */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          backgroundImage: `
            linear-gradient(#333 1px, transparent 1px),
            linear-gradient(90deg, #333 1px, transparent 1px)
          `,
          backgroundSize: `${GRID * canvas.scale}px ${GRID * canvas.scale}px`,
          backgroundPosition: `${canvas.x}px ${canvas.y}px`,
        }}
      />

      {/* ================= World ================= */}
      <div className="absolute inset-0" onPointerDown={onCanvasPointerDown}>
        {/* GPU 加速的连接线层 */}
        <canvas
          ref={edgeCanvasRef}
          className="absolute inset-0 pointer-events-none"
        />

        <div
          style={{
            transform: `translate(${canvas.x}px, ${canvas.y}px) scale(${canvas.scale})`,
            transformOrigin: "0 0",
          }}
        >
          {nodes.map((node) => (
            <Node
              key={node.id}
              node={node}
              scale={canvas.scale}
              activePinId={activePin?.id}
              onPointerDown={onNodePointerDown}
              onAddInput={handleNodeAddInput}
              onPinClick={handlePinClick}
              onPinPointerDown={onPinPointerDown}
            />
          ))}
        </div>
      </div>
      {/* ================= HUD ================= */}
      <HUD />

      {/* ================= Selection Box ================= */}
      {selection && ref.current && (
        <div
          className="absolute border border-blue-500 bg-blue-500/20 pointer-events-none z-50"
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
      {contextMenu?.visible && (
        <NodePalette
          x={contextMenu.x}
          y={contextMenu.y}
          onSelect={handleNodePaletteSelect}
          filterPin={pendingConnection}
        />
      )}

      {/* ================= Variable Drop Menu ================= */}
      {variableDropMenu && (
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
                canvas.scale,
                "get_variable",
                {
                  title: `Get ${variableDropMenu.variableName}`,
                  variableId: variableDropMenu.variableId,
                  variableType: variableDropMenu.variableType // 传入初始变量类型
                }
              );
              if (newNode) setNodes((prev) => [...prev, newNode]);
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
                canvas.scale,
                "set_variable",
                {
                  title: `Set ${variableDropMenu.variableName}`,
                  variableId: variableDropMenu.variableId,
                  variableType: variableDropMenu.variableType // 传入初始变量类型
                }
              );
              if (newNode) setNodes((prev) => [...prev, newNode]);
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
