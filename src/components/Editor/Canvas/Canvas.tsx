import { useRef, useState, useEffect, useCallback, useMemo, useLayoutEffect } from "react";
import { Node } from "../Nodes/Node";
import { Pin } from "../Types/nodes";
import { useDrag } from "../Context/DragContext";
import { useCanvas } from "../Context/CanvasContext";
import { useTheme } from "../Context/ThemeContext";
import { useViewportStore } from "../Store/useViewportStore";
import { useNodeStore } from "../Store/useNodeStore";
import { useGestureStore } from "../Store/useGestureStore";
import { createNodeFromTemplate } from "../Utils/nodeUtils";
import { createInternalNode } from "../Utils/internalNodes";
import { ConnectionLine } from "./ConnectionLine";
import { useBackendNodeCreation } from "../Hooks/useBackendNodeCreation";

// Extracted Components
import { ViewportGrid } from "./ViewportGrid";
import { TransformContainer } from "./TransformContainer";
import { EdgesLayer } from "./EdgesLayer";
import CanvasOverlays from "./CanvasOverlays";
import { DEFAULT_VIEWPORT } from "./constants";

/* ================= Canvas Components ================= */

export default function Canvas() {
  const {
    nodes,
    setCanvas,
    setNodes,
    onCanvasPointerDown,
    onNodePointerDown,
    onPinPointerDown,

    contextMenu,
    setContextMenu,
    variables,
    globalVariables,
    saveHistory,
    activeTabId,
    pendingConnection,
    setPendingConnection,
    functions,
    macros,
    connectPins,
    groupId,
    selectedNodeIds
  } = useCanvas();
  const { theme } = useTheme();
  const { drag } = useDrag();
  const gesture = useGestureStore(state => state.gesture);
  const { createNode } = useBackendNodeCreation();

  const scale = useViewportStore(useCallback(state => state.viewports[groupId]?.scale || 1, [groupId]));

  // --- 视口裁剪 (Culling) 逻辑 ---
  const [visibleNodeIds, setVisibleNodes] = useState<Set<string>>(new Set());

  const ref = useRef<HTMLDivElement>(null);

  const updateVisibleNodes = useCallback(() => {
    const el = ref.current;
    if (!el) return;

    // 1. 获取画布在浏览器窗口中的实际位置和尺寸
    const rect = el.getBoundingClientRect();

    // 2. 直接从 Store 获取最新状态，避免闭包过时
    const viewport = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
    const currentTabId = activeTabId || "";
    const allNodes = useNodeStore.getState().getNodes(currentTabId);

    // 3. 计算视口边界（世界坐标系）
    // 核心逻辑：(屏幕边界点 - 容器偏移 - 平移量) / 缩放
    const padding = 200 / viewport.scale; // 适当减少 padding 以提升性能

    const worldViewLeft = -viewport.x / viewport.scale - padding;
    const worldViewTop = -viewport.y / viewport.scale - padding;
    const worldViewRight = (rect.width - viewport.x) / viewport.scale + padding;
    const worldViewBottom = (rect.height - viewport.y) / viewport.scale + padding;

    const visible = new Set<string>();

    allNodes.forEach(node => {
      // 矩形相交判定
      // 假设节点尺寸，如果有真实测量值更好
      const nodeWidth = 300;
      const nodeHeight = 300;

      const isVisible = (
        node.position.x + nodeWidth > worldViewLeft &&
        node.position.x < worldViewRight &&
        node.position.y + nodeHeight > worldViewTop &&
        node.position.y < worldViewBottom
      );

      if (isVisible) {
        visible.add(node.id);
      }
    });

    setVisibleNodes(visible);
  }, [groupId, activeTabId]); // 移除了 nodes 依赖，仅通过 Store 获取

  // 当节点列表变化、缩放变化或平移结束时更新裁剪

  useEffect(() => {
    updateVisibleNodes();
  }, [scale, nodes, updateVisibleNodes]);

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
  }, [setCanvas, groupId]);

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

  const prevDragRef = useRef(drag);

  const pinIndex = useMemo(() => {
    const map = new Map<string, Pin>();

    nodes.forEach(node => {
      node.inputs.forEach((pin: Pin) => map.set(pin.id, pin));
      node.outputs.forEach((pin: Pin) => map.set(pin.id, pin));
    });
    return map;
  }, [nodes]);

  const pinNodeIdIndex = useMemo(() => {
    const map = new Map<string, string>();
    nodes.forEach(node => {
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
    const currentNodes = useNodeStore.getState().getNodes(activeTabId || "");

    currentNodes.forEach(node => {
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
  }, [activeTabId, scale, visibleNodeIds, nodes]);

  // 获取 Pin 的 world 坐标
  const getPinWorldPos = useCallback((pinId: string) => {
    const nodeId = pinNodeIdIndex.get(pinId);
    if (!nodeId) return null;
    const tabNodes = useNodeStore.getState().getNodes(activeTabId || "");
    const node = tabNodes.find(n => n.id === nodeId);
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


  // 3. 动态添加输入
  const handleNodeAddInput = useCallback((id: string) => {
    saveHistory();
    setNodes((prev) =>
      prev.map((node) => {
        if (node.id === id) {
          const newNode = node.clone();
          const newIndex = newNode.inputs.length;
          newNode.addInput({
            id: `${id}_input_${newIndex}_${Date.now()}`,
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
  }, [drag, variables]);

  /* ===== Data Drag ===== */
  async function handleDropTemplate(dragState: any, event: MouseEvent | PointerEvent) {
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

    // 检查是否落在了 pin 上 (需求 3: 拖动变量/数据到赋值框)
    const elements = document.elementsFromPoint(dragState.x, dragState.y);
    const pinEl = elements.find(e => e.closest("[data-pin-id]"))?.closest("[data-pin-id]");
    const targetPinId = pinEl?.getAttribute("data-pin-id");

    // 如果是数据 (DataFrame 或 Column)
    if (dragState.template.category === "Data") {
      const newNode = createNodeFromTemplate({ x, y }, currentCanvas.scale, dragState.template.type, {
        variableId: dragState.template.variableId,
        variableName: dragState.template.variableName,
        initialData: dragState.template.initialData
      });
      if (newNode) {
        // 使用后端 API 创建节点（等待后端返回）
        const createdNode = await createNode(newNode);

        if (createdNode && targetPinId) {
          const outputPin = createdNode.outputs[0];
          if (outputPin) {
            connectPins(outputPin.id, targetPinId);
          }
        }
      }
      return;
    }

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
        const newNode = createNodeFromTemplate({ x, y }, currentCanvas.scale, spawnType, {
          variableId: dragState.template.variableId,
          variableName: dragState.template.variableName,
          variableType: dragState.template.variableType,
          variableIsArray: dragState.template.variableIsArray
        } as any);
        if (newNode) {
          // 使用后端 API 创建节点（等待后端返回）
          const createdNode = await createNode(newNode);

          if (createdNode && targetPinId && spawnType === "get_variable") {
            const outputPin = createdNode.outputs[0];
            if (outputPin) {
              connectPins(outputPin.id, targetPinId);
            }
          }
        }
        return;
      }

      if (targetPinId) {
        const newNode = createNodeFromTemplate({ x, y }, currentCanvas.scale, "get_variable", {
          variableId: dragState.template.variableId,
          variableName: dragState.template.variableName,
          variableType: dragState.template.variableType,
          variableIsArray: dragState.template.variableIsArray
        } as any);
        if (newNode) {
          // 使用后端 API 创建节点（等待后端返回）
          const createdNode = await createNode(newNode);

          if (createdNode) {
            const outputPin = createdNode.outputs[0];
            if (outputPin) {
              connectPins(outputPin.id, targetPinId);
            }
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
        variableIsArray: dragState.template.variableIsArray,
      });
      return;
    } else if (dragState.template.type === "call_function" || dragState.template.type === "call_macro") {
      const type = dragState.template.type;
      const subId = dragState.template.subGraphId;
      const subName = dragState.template.subName;
      const subData = (type === 'call_function') ? functions[subId] : macros[subId];
      if (!subData) return;

      const node = createInternalNode(
        `node-${crypto.randomUUID()}`,
        type,
        subName,
        type === 'call_function' ? ["Functions"] : ["Macros"],
        { x, y },
        [{ id: `exec-in-${Date.now()}`, nodeId: "", name: "In", type: "exec", direction: "input", links: [] },
        ...(subData.inputs || []).map(p => ({ id: `in-${p.id}-${Date.now()}`, nodeId: "", name: p.name, type: p.type as any, direction: "input", links: [] as string[], isArray: p.isArray }))],
        [{ id: `exec-out-${Date.now()}`, nodeId: "", name: "Out", type: "exec", direction: "output", links: [] },
        ...(subData.outputs || []).map(p => ({ id: `out-${p.id}-${Date.now()}`, nodeId: "", name: p.name, type: p.type as any, direction: "output", links: [] as string[], isArray: p.isArray }))],
        false
      );
      node.subGraphId = subId;

      // 使用后端 API 创建节点（等待后端返回）
      await createNode(node);
      return;
    }

    const newNode = createNodeFromTemplate(
      { x, y },
      currentCanvas.scale,
      dragState.template.type
    );
    if (newNode) {
      // 使用后端 API 创建节点（等待后端返回）
      await createNode(newNode);
    }
  }

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();

    // 检查点击来源：如果是 UI 组件，则忽略
    const target = e.target as HTMLElement;
    if (
      target.closest(".menubar-container") ||
      target.closest(".sidebar-container") ||
      target.closest(".menu-container") ||
      target.closest(".hud-container")
    ) {
      return;
    }

    setContextMenu({
      x: e.clientX,
      y: e.clientY,
      visible: true,
    });
  }, [setContextMenu]);

  const activePin = useMemo(() => {
    if (gesture?.type === "connect") return gesture.startPin;
    if (pendingConnection && contextMenu?.visible) return pendingConnection;
    return null;
  }, [gesture, pendingConnection, contextMenu]);


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
        onContextMenu={handleContextMenu}
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
          activeTabId={activeTabId} // Pass activeTabId
          theme={theme}
        />

        {/* Optimized Connection Line */}
        <ConnectionLine
          groupId={groupId}
          getPinWorldPos={getPinWorldPos}
          getCanvasLocalPoint={getCanvasLocalPoint}
          pendingConnection={pendingConnection}
          menuPos={contextMenu}
        />

        <TransformContainer groupId={groupId}>
          {nodes.filter(n => visibleNodeIds.has(n.id)).map((node) => (
            <Node
              key={node.id}
              id={node.id}
              node={node}
              scale={scale}
              selected={selectedNodeIdsSet.has(node.id)}
              activePinId={activePin?.id}
              onPointerDown={(id, e) => onNodePointerDown(id, e)}
              onAddInput={handleNodeAddInput}
              onPinClick={handlePinClick}
              onPinPointerDown={(e, p) => onPinPointerDown(p.id, e)}
            />
          ))}
        </TransformContainer>
      </div>

      <CanvasOverlays
        canvasRef={ref}
        variableDropMenu={variableDropMenu}
        setVariableDropMenu={setVariableDropMenu}
      />
    </div>
  );
}
