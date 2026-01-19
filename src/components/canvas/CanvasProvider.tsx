import React, { useRef, useState, useCallback, useEffect } from "react";
import { CanvasContext } from "./CanvasContext";
import { CanvasState, Gesture } from "./type";
import { clamp } from "../../types";
import { Pin, BaseNode } from "../node/models";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import { invoke } from "@tauri-apps/api/core";
import { serializeGraph, deserializeGraph } from "./io";
import { useUI } from "../ui/UIProvider";

export const CanvasProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const { showToast } = useUI();

  const [nodes, setNodes] = useState<BaseNode[]>([]);
  const nodesRef = useRef(nodes);
  
  useEffect(() => {
    nodesRef.current = nodes;
  }, [nodes]);

  const [canvas, setCanvas] = useState<CanvasState>({
    x: 0,
    y: 0,
    scale: 1,
  });

  const [variables, setVariables] = useState<Record<string, { name: string; type: string; value: any }>>({
    "default_var": { name: "default_var", type: "int", value: 0 }
  });
  const variablesRef = useRef(variables);
  useEffect(() => {
    variablesRef.current = variables;
  }, [variables]);

  const [selectedVariableId, setSelectedVariableId] = useState<string | null>(null);
  const [currentPath, setCurrentPath] = useState<string | null>(null);

  // 需求 4: 画布中心固定逻辑
  useEffect(() => {
    let lastWidth = window.innerWidth;
    let lastHeight = window.innerHeight;

    const handleResize = () => {
      const dw = window.innerWidth - lastWidth;
      const dh = window.innerHeight - lastHeight;
      
      // 为了保持中心不变，我们需要补偿偏移量的一半
      setCanvas(prev => ({
        ...prev,
        x: prev.x + dw / 2,
        y: prev.y + dh / 2
      }));

      lastWidth = window.innerWidth;
      lastHeight = window.innerHeight;
    };

    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  const addVariable = useCallback((name: string, type: string) => {
    const id = `var-${crypto.randomUUID()}`;
    setVariables(prev => ({
      ...prev,
      [id]: { name, type, value: type === "int" ? 0 : type === "bool" ? false : type === "float" ? 0.0 : "" }
    }));
    setSelectedVariableId(id); // 自动选中新创建的变量
  }, []);

  const updateVariable = useCallback((id: string, data: Partial<{ name: string; type: string; value: any }>) => {
    setVariables(prev => ({
      ...prev,
      [id]: { ...prev[id], ...data }
    }));

    // 同步更新画布上相关的节点
    if (data.name || data.type) {
      setNodes(prevNodes => {
        let nodesChanged = false;
        const nextNodes = prevNodes.map(node => {
          if (node.variableId === id) {
            nodesChanged = true;
            const newNode = node.clone();
            
            // 1. 同步名称/标题
            if (data.name) {
              const prefix = node.type === "get_variable" ? "Get" : "Set";
              newNode.title = `${prefix} ${data.name}`;
            }

            // 2. 同步类型
            if (data.type) {
              // 更新所有非执行流针脚的类型
              const updatePins = (pins: Pin[]) => {
                pins.forEach(p => {
                  if (p.type !== "exec") {
                    p.type = data.type as any;
                    // 注意：类型改变后，原有的连接可能变得无效
                    // 这里简单处理：保留连接，让用户在连接时看到颜色不匹配
                    // 或者可以更激进地：p.links = [];
                  }
                });
              };
              updatePins(newNode.inputs);
              updatePins(newNode.outputs);
            }
            
            return newNode;
          }
          return node;
        });
        return nodesChanged ? nextNodes : prevNodes;
      });
    }
  }, []);

  const deleteVariable = useCallback((id: string) => {
    setVariables(prev => {
      const next = { ...prev };
      delete next[id];
      return next;
    });

    setNodes(prevNodes => {
      // 1. 收集所有要删除节点（关联该变量的节点）的 Pin ID
      const pinsToDelete = new Set<string>();
      prevNodes.forEach((node) => {
        if (node.variableId === id) {
          node.inputs.forEach((p) => pinsToDelete.add(p.id));
          node.outputs.forEach((p) => pinsToDelete.add(p.id));
        }
      });

      // 2. 过滤掉关联该变量的节点，并清理剩余节点中的连接关系
      return prevNodes
        .filter((node) => node.variableId !== id)
        .map((node) => {
          let nodeChanged = false;
          const newNode = node.clone();

          const cleanPins = (pins: Pin[]) => {
            pins.forEach(pin => {
              const originalLength = pin.links.length;
              pin.links = pin.links.filter(linkId => !pinsToDelete.has(linkId));
              if (pin.links.length !== originalLength) {
                nodeChanged = true;
              }
            });
          };

          cleanPins(newNode.inputs);
          cleanPins(newNode.outputs);

          return nodeChanged ? newNode : node;
        });
    });
  }, []);

  const exportGraph = useCallback(async () => {
    try {
      const data = serializeGraph(nodesRef.current, canvas, variablesRef.current);
      const path = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: currentPath || `yssbi-graph-${Date.now()}.json`
      });

      if (path) {
        await writeTextFile(path, JSON.stringify(data, null, 2));
        setCurrentPath(path);
      }
    } catch (e) {
      console.error("Export failed:", e);
    }
  }, [canvas, currentPath]);

  const saveGraph = useCallback(async () => {
    if (!currentPath) {
      return exportGraph();
    }
    try {
      const data = serializeGraph(nodesRef.current, canvas, variablesRef.current);
      await writeTextFile(currentPath, JSON.stringify(data, null, 2));
      console.log("Graph saved to:", currentPath);
    } catch (e) {
      console.error("Save failed:", e);
      // If save fails (e.g. file moved), fallback to Export
      exportGraph();
    }
  }, [canvas, currentPath, exportGraph]);

  const importGraph = useCallback(async (json?: string) => {
    try {
      let content = json;
      let path: string | null = null;
      if (!content) {
        const selected = await open({
          multiple: false,
          filters: [{ name: "JSON", extensions: ["json"] }]
        });
        if (!selected || Array.isArray(selected)) return;
        path = selected as string;
        content = await readTextFile(path);
      }

      if (!content) return;
      const { nodes: newNodes, canvas: newCanvas, variables: newVars } = deserializeGraph(JSON.parse(content));
      setNodes(newNodes);
      setCanvas(newCanvas);
      if (newVars) setVariables(newVars);
      if (path) setCurrentPath(path);
    } catch (e) {
      console.error("Import failed:", e);
    }
  }, []);

  const executeGraph = useCallback(async () => {
    try {
      showToast("开始执行...", "info", 1000);
      const data = serializeGraph(nodesRef.current, canvas, variablesRef.current);
      const result = await invoke<string[]>("execute_graph", { data });
      
      // 显示执行日志中的最后几条关键消息，或者显示全部
      result.forEach(log => {
        if (log.includes("[NODE PRINT]")) {
          showToast(log, "success", 5000);
        } else if (log.includes("[Error]")) {
          showToast(log, "error", 5000);
        }
      });

      if (result.length > 0) {
        const lastLog = result[result.length - 1];
        if (!lastLog.includes("[Error]")) {
          showToast("执行完成", "success", 2000);
        }
      }
    } catch (e) {
      console.error("Execution failed:", e);
      showToast(`执行失败: ${e}`, "error", 5000);
    }
  }, [canvas, showToast]);

  const [gesture, setGesture] = useState<Gesture>(null);
  const gestureRef = useRef<Gesture>(null);

  const [selection, setSelection] = useState<{
    startX: number;
    startY: number;
    currentX: number;
    currentY: number;
  } | null>(null);

  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    visible: boolean;
  } | null>(null);

  /* ================= Wheel Zoom ================= */

  const onCanvasWheel = useCallback(
    (e: React.WheelEvent) => {
      // 这里的 stopPropagation 已经由 Menubar/Sidebar 处理，
      // 更加彻底地检查点击来源，如果是任何具有 UI 标识的组件则拦截
      const target = e.target as HTMLElement;
      if (
        target.closest(".menubar-container") || 
        target.closest(".sidebar-container") ||
        target.closest(".menu-container") ||
        target.closest(".hud-container")
      ) {
        return;
      }

      e.preventDefault();

      const factor = 0.001;
      const nextScale = clamp(canvas.scale * (1 - e.deltaY * factor), 0.2, 4);

      const rect = e.currentTarget.getBoundingClientRect();
      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;

      const worldX = (mouseX - canvas.x) / canvas.scale;
      const worldY = (mouseY - canvas.y) / canvas.scale;

      setCanvas({
        scale: nextScale,
        x: mouseX - worldX * nextScale,
        y: mouseY - worldY * nextScale,
      });
    },
    [canvas]
  );

  /* ================= Pointer Move ================= */

  const onCanvasPointerMove = useCallback((e: PointerEvent) => {
    const currentGesture = gestureRef.current;
    if (!currentGesture) return;

    if (currentGesture.type === "pan") {
      const dx = e.clientX - currentGesture.lastX;
      const dy = e.clientY - currentGesture.lastY;

      if (Math.abs(dx) > 2 || Math.abs(dy) > 2) {
        currentGesture.moved = true;
      }

      setCanvas((prev) => ({
        ...prev,
        x: prev.x + dx,
        y: prev.y + dy,
      }));

      currentGesture.lastX = e.clientX;
      currentGesture.lastY = e.clientY;
      setGesture({ ...currentGesture });
    } else if (currentGesture.type === "select") {
      currentGesture.currentX = e.clientX;
      currentGesture.currentY = e.clientY;
      setSelection({ ...currentGesture });
      setGesture({ ...currentGesture });
    } else if (currentGesture.type === "connect") {
      currentGesture.currentX = e.clientX;
      currentGesture.currentY = e.clientY;
      setGesture({ ...currentGesture });
    }
  }, []);

  /* ================= Pointer Up ================= */

  const onCanvasPointerUp = useCallback((e: PointerEvent) => {
    const currentGesture = gestureRef.current;

    if (currentGesture?.type === "select") {
      setSelection(null);
    } else if (currentGesture?.type === "pan" && !currentGesture.moved && e.button === 2) {
      setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
    } else if (currentGesture?.type === "connect") {
      // Logic handled in Canvas.tsx via gesture state
    }

    gestureRef.current = null;
    setGesture(null);
    window.removeEventListener("pointermove", onCanvasPointerMove);
    window.removeEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove]);

  /* ================= Pin Pointer Down ================= */
  const onPinPointerDown = useCallback((e: React.PointerEvent, pin: Pin) => {
    e.stopPropagation();
    e.preventDefault();

    if (e.altKey) {
      setGesture({ type: "disconnect", pin });
      return;
    }

    const start = {
      type: "connect" as const,
      startPin: pin,
      startX: e.clientX,
      startY: e.clientY,
      currentX: e.clientX,
      currentY: e.clientY,
      isReconnect: e.ctrlKey,
    };

    gestureRef.current = start;
    setGesture(start);

    window.addEventListener("pointermove", onCanvasPointerMove);
    window.addEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove, onCanvasPointerUp]);

  /* ================= Pointer Down ================= */

  const onCanvasPointerDown = useCallback((e: React.PointerEvent) => {
    // 需求 5: 点击画布背景时清除变量选中状态
    setSelectedVariableId(null);

    if (e.button === 0) {
      // 左键开始选择
      const start = {
        type: "select" as const,
        startX: e.clientX,
        startY: e.clientY,
        currentX: e.clientX,
        currentY: e.clientY,
      };
      gestureRef.current = start;
      setSelection(start);
      setGesture(start);
    } else if (e.button === 1 || e.button === 2) {
      // 中键或右键开始平移
      const start = {
        type: "pan" as const,
        lastX: e.clientX,
        lastY: e.clientY,
        moved: false,
      };
      gestureRef.current = start;
      setGesture(start);
    }

    window.addEventListener("pointermove", onCanvasPointerMove);
    window.addEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove, onCanvasPointerUp]);

  return (
    <CanvasContext.Provider
      value={{
        canvas,
        setCanvas,
        nodes,
        setNodes,
        onCanvasWheel,
        onCanvasPointerDown,
        onPinPointerDown,
        selection,
        gesture,
        setGesture,
        contextMenu,
        setContextMenu,
        exportGraph,
        saveGraph,
        importGraph,
        executeGraph,
        variables,
        selectedVariableId,
        setSelectedVariableId,
        addVariable,
        updateVariable,
        deleteVariable,
      }}
    >
      {children}
    </CanvasContext.Provider>
  );
};
