import React, { useRef, useState, useCallback, useEffect, useMemo } from "react";
import { CanvasContext } from "./CanvasContext";
import { CanvasState, Gesture, Tab } from "./type";
import { clamp } from "../../types";
import { Pin, BaseNode } from "../node/models";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import { invoke } from "@tauri-apps/api/core";
import { serializeGraph, deserializeGraph } from "./io";
import { useUI } from "../ui/UIProvider";
import { NODE_REGISTRY } from "../node/registry";

/* ================= Helper Functions ================= */

const isSingleLinkPin = (p: Pin) => p.type === "exec" || p.direction === "input";

const isCompatiblePins = (a: Pin, b: Pin) => {
  if (a.direction === b.direction) return false;
  if (a.type === b.type) return true;

  // Wildcard type 'object' can connect to any data pin, but not 'exec'
  if (a.type === "exec" || b.type === "exec") return false;
  if (a.type === "object" || b.type === "object") return true;

  // Allow int to float connection (standard casting)
  if ((a.type === "int" && b.type === "float") || (a.type === "float" && b.type === "int")) return true;

  return false;
};

const updatePinLink = (node: BaseNode, pId: string, oId: string) => {
  const p = [...node.inputs, ...node.outputs].find((x) => x.id === pId);
  if (!p) return false;
  if (isSingleLinkPin(p)) {
    if (p.links.length === 1 && p.links[0] === oId) return false;
    p.links = [oId];
  } else {
    if (p.links.includes(oId)) return false;
    p.links = [...p.links, oId];
  }
  return true;
};

const removePinLink = (node: BaseNode, pId: string, oId: string) => {
  const p = [...node.inputs, ...node.outputs].find((x) => x.id === pId);
  if (!p || !p.links.includes(oId)) return false;
  p.links = p.links.filter((id) => id !== oId);
  return true;
};

interface TabData {
  nodes: BaseNode[];
  canvas: CanvasState;
  variables: Record<string, any>;
  history: { past: any[]; future: any[] };
  currentPath: string | null;
}

export const CanvasProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const { showToast } = useUI();

  // --- Active Tab State ---
  const [nodes, setNodes] = useState<BaseNode[]>([]);
  const nodesRef = useRef(nodes);
  useEffect(() => { nodesRef.current = nodes; }, [nodes]);

  const [canvas, setCanvas] = useState<CanvasState>({ x: 0, y: 0, scale: 1 });

  const [variables, setVariables] = useState<Record<string, { name: string; type: string; value: any }>>({
    "default_var": { name: "default_var", type: "int", value: 0 }
  });
  const variablesRef = useRef(variables);
  useEffect(() => { variablesRef.current = variables; }, [variables]);

  const [globalVariables, setGlobalVariables] = useState<Record<string, { name: string; type: string; value: any }>>({});
  const globalVariablesRef = useRef(globalVariables);
  useEffect(() => { globalVariablesRef.current = globalVariables; }, [globalVariables]);

  const [selectedVariableId, setSelectedVariableId] = useState<string | null>(null);
  const [currentPath, setCurrentPath] = useState<string | null>(null);

  const [history, setHistory] = useState<{ past: any[]; future: any[] }>({ past: [], future: [] });

  const [functions, setFunctions] = useState<Record<string, { name: string }>>({});
  const functionsRef = useRef(functions);
  useEffect(() => { functionsRef.current = functions; }, [functions]);

  const [macros, setMacros] = useState<Record<string, { name: string }>>({});
  const macrosRef = useRef(macros);
  useEffect(() => { macrosRef.current = macros; }, [macros]);

  const pinIndex = useMemo(() => {
    const map = new Map<string, Pin>();
    nodes.forEach((node) => {
      node.inputs.forEach((pin) => map.set(pin.id, pin));
      node.outputs.forEach((pin) => map.set(pin.id, pin));
    });
    return map;
  }, [nodes]);

  const pinNodeIdIndex = useMemo(() => {
    const map = new Map<string, string>();
    nodes.forEach((node) => {
      node.inputs.forEach((pin) => map.set(pin.id, node.id));
      node.outputs.forEach((pin) => map.set(pin.id, node.id));
    });
    return map;
  }, [nodes]);

  // 使用 Ref 访问最新的索引，避免 callback 闭包陈旧
  const pinIndexRef = useRef(pinIndex);
  useEffect(() => { pinIndexRef.current = pinIndex; }, [pinIndex]);
  const pinNodeIdIndexRef = useRef(pinNodeIdIndex);
  useEffect(() => { pinNodeIdIndexRef.current = pinNodeIdIndex; }, [pinNodeIdIndex]);

  // --- Tabs Management ---
  const [tabs, setTabs] = useState<Tab[]>([
    { id: "default", title: "Untitled", path: null, isDirty: false }
  ]);
  const [activeTabId, setActiveTabId] = useState<string | null>("default");
  const tabDataRef = useRef<Record<string, TabData>>({});

  const syncCurrentTabToData = useCallback(() => {
    if (!activeTabId) return;
    tabDataRef.current[activeTabId] = {
      nodes: nodesRef.current,
      canvas,
      variables: variablesRef.current,
      history,
      currentPath
    };
  }, [activeTabId, canvas, history, currentPath]);

  const handleSetActiveTabId = useCallback((newId: string) => {
    if (newId === activeTabId) return;

    syncCurrentTabToData();

    const nextData = tabDataRef.current[newId];
    if (nextData) {
      setNodes(nextData.nodes);
      setCanvas(nextData.canvas);
      setVariables(nextData.variables);
      setHistory(nextData.history);
      setCurrentPath(nextData.currentPath);
    } else {
      setNodes([]);
      setCanvas({ x: 0, y: 0, scale: 1 });
      setVariables({ "default_var": { name: "default_var", type: "int", value: 0 } });
      setHistory({ past: [], future: [] });
      setCurrentPath(null);
    }
    setActiveTabId(newId);
  }, [activeTabId, syncCurrentTabToData]);

  const addTab = useCallback((title: string = "Untitled", data?: TabData) => {
    const id = `tab-${crypto.randomUUID()}`;
    // Initialize isDirty to false for new tabs
    const newTab: Tab = { id, title, path: data?.currentPath || null, isDirty: false };
    if (data) tabDataRef.current[id] = data;
    setTabs(prev => [...prev, newTab]);
    handleSetActiveTabId(id);
    return id;
  }, [handleSetActiveTabId]);

  const closeTab = useCallback((id: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    setTabs(prev => {
      if (prev.length <= 1) {
        showToast("无法关闭最后一个标签页", "info", 2000);
        return prev;
      }
      const index = prev.findIndex(t => t.id === id);
      const newTabs = prev.filter(t => t.id !== id);
      delete tabDataRef.current[id];
      if (id === activeTabId) {
        const nextActiveIndex = Math.min(index, newTabs.length - 1);
        const nextId = newTabs[nextActiveIndex].id;
        setTimeout(() => handleSetActiveTabId(nextId), 0);
      }
      return newTabs;
    });
  }, [activeTabId, handleSetActiveTabId, showToast]);

  /* ================= Clipboard ================= */
  const clipboardRef = useRef<BaseNode[]>([]);

  /* ================= History (Undo/Redo) ================= */
  const markDirty = useCallback((dirty: boolean) => {
    setTabs(prev => prev.map(t => {
      if (t.id === activeTabId) {
        return { ...t, isDirty: dirty };
      }
      return t;
    }));
  }, [activeTabId]);

  const saveHistory = useCallback(() => {
    const currentState = serializeGraph(nodesRef.current, canvas, variablesRef.current, globalVariablesRef.current, functionsRef.current, macrosRef.current);
    setHistory(prev => {
      if (prev.past.length > 0) {
        const lastState = prev.past[prev.past.length - 1];
        if (JSON.stringify(lastState) === JSON.stringify(currentState)) return prev;
      }
      return { past: [...prev.past, currentState].slice(-50), future: [] };
    });
    markDirty(true);
  }, [canvas, markDirty]);

  const undo = useCallback(() => {
    setHistory(prev => {
      if (prev.past.length === 0) return prev;
      const newPast = [...prev.past];
      const previousState = newPast.pop()!;
      const currentState = serializeGraph(nodesRef.current, canvas, variablesRef.current, globalVariablesRef.current, functionsRef.current, macrosRef.current);
      const { nodes: newNodes, canvas: newCanvas, localVariables: newVars, globalVariables: newGlobalVars, functions: newFuncs, macros: newMacros } = deserializeGraph(previousState);
      setNodes(newNodes);
      setCanvas(newCanvas);
      setVariables(newVars);
      setGlobalVariables(newGlobalVars);
      setFunctions(newFuncs);
      setMacros(newMacros);
      return { past: newPast, future: [currentState, ...prev.future] };
    });
    markDirty(true);
  }, [canvas, markDirty]);

  const redo = useCallback(() => {
    setHistory(prev => {
      if (prev.future.length === 0) return prev;
      const newFuture = [...prev.future];
      const nextState = newFuture.shift()!;
      const currentState = serializeGraph(nodesRef.current, canvas, variablesRef.current, globalVariablesRef.current, functionsRef.current, macrosRef.current);
      const { nodes: newNodes, canvas: newCanvas, localVariables: newVars, globalVariables: newGlobalVars, functions: newFuncs, macros: newMacros } = deserializeGraph(nextState);
      setNodes(newNodes);
      setCanvas(newCanvas);
      setVariables(newVars);
      setGlobalVariables(newGlobalVars);
      setFunctions(newFuncs);
      setMacros(newMacros);
      return { past: [...prev.past, currentState], future: newFuture };
    });
    markDirty(true);
  }, [canvas, markDirty]);

  const copy = useCallback(() => {
    const selectedNodes = nodesRef.current.filter(n => n.selected);
    if (selectedNodes.length === 0) return;
    clipboardRef.current = selectedNodes.map(n => n.clone());
  }, []);

  const paste = useCallback((pos?: { x: number; y: number }) => {
    let clipboard = clipboardRef.current;
    if (clipboard.length === 0) return;
    clipboard = clipboard.filter(node => {
      if (!NODE_REGISTRY.getDefinition(node.type)) return false;
      if (node.variableId && !variablesRef.current[node.variableId] && !globalVariablesRef.current[node.variableId]) return false;
      return true;
    });
    if (clipboard.length === 0) return;
    let targetX = pos ? pos.x : -canvas.x / canvas.scale + window.innerWidth / 2 / canvas.scale;
    let targetY = pos ? pos.y : -canvas.y / canvas.scale + window.innerHeight / 2 / canvas.scale;
    let minX = Infinity, minY = Infinity;
    clipboard.forEach(node => {
      minX = Math.min(minX, node.position.x);
      minY = Math.min(minY, node.position.y);
    });
    const offsetX = targetX - minX, offsetY = targetY - minY;
    const pinIdMap = new Map<string, string>();
    const newNodes = clipboard.map(node => {
      const newNode = node.clone();
      const newNodeId = `node-${crypto.randomUUID()}`;
      newNode.id = newNodeId;
      newNode.position = { x: node.position.x + offsetX, y: node.position.y + offsetY };
      newNode.selected = true;
      const updatePins = (pins: Pin[]) => {
        return pins.map(pin => {
          const oldId = pin.id;
          const suffix = oldId.split('-').pop();
          const newId = `${newNodeId}-${pin.direction}-${suffix}-${crypto.randomUUID().slice(0, 8)}`;
          pin.id = newId; pin.nodeId = newNodeId; pinIdMap.set(oldId, newId);
          return pin;
        });
      };
      newNode.inputs = updatePins(newNode.inputs);
      newNode.outputs = updatePins(newNode.outputs);
      return newNode;
    });
    newNodes.forEach(node => {
      [...node.inputs, ...node.outputs].forEach(pin => {
        pin.links = pin.links.map(oldLinkId => pinIdMap.get(oldLinkId)).filter((newLinkId): newLinkId is string => !!newLinkId);
      });
    });
    saveHistory();
    setNodes(prev => {
      const next = prev.map(n => {
        if (n.selected) {
          const cloned = n.clone(); cloned.selected = false; return cloned;
        }
        return n;
      });
      return [...next, ...newNodes];
    });
  }, [canvas, saveHistory]);

  const deleteSelected = useCallback(() => {
    const selectedIds = new Set(nodesRef.current.filter(n => n.selected).map(n => n.id));
    if (selectedIds.size === 0) return;
    saveHistory();
    setNodes((prev) => {
      const pinsToDelete = new Set<string>();
      prev.forEach((node) => {
        if (selectedIds.has(node.id)) {
          node.inputs.forEach((p) => pinsToDelete.add(p.id));
          node.outputs.forEach((p) => pinsToDelete.add(p.id));
        }
      });
      return prev.filter((node) => !selectedIds.has(node.id)).map((node) => {
        let nodeChanged = false;
        const newNode = node.clone();
        const cleanPins = (pins: Pin[]) => {
          pins.forEach(pin => {
            const originalLength = pin.links.length;
            pin.links = pin.links.filter(linkId => !pinsToDelete.has(linkId));
            if (pin.links.length !== originalLength) nodeChanged = true;
          });
        };
        cleanPins(newNode.inputs); cleanPins(newNode.outputs);
        return nodeChanged ? newNode : node;
      });
    });
  }, [saveHistory]);

  const cut = useCallback(() => { copy(); deleteSelected(); }, [copy, deleteSelected]);

  const connectPins = useCallback((pinAId: string, pinBId: string) => {
    const pinA = pinIndexRef.current.get(pinAId);
    const pinB = pinIndexRef.current.get(pinBId);
    if (!pinA || !pinB || !isCompatiblePins(pinA, pinB)) return;

    const outputPin = pinA.direction === "output" ? pinA : pinB;
    const inputPin = pinA.direction === "input" ? pinA : pinB;
    const outputNodeId = pinNodeIdIndexRef.current.get(outputPin.id);
    const inputNodeId = pinNodeIdIndexRef.current.get(inputPin.id);
    if (!outputNodeId || !inputNodeId) return;

    saveHistory();
    setNodes((prev) => {
      // 记录需要清理的旧连接
      const toCleanup: { pinId: string; oldPeerId: string }[] = [];
      if (isSingleLinkPin(outputPin) && outputPin.links.length > 0) {
        toCleanup.push({ pinId: outputPin.id, oldPeerId: outputPin.links[0] });
      }
      if (isSingleLinkPin(inputPin) && inputPin.links.length > 0) {
        toCleanup.push({ pinId: inputPin.id, oldPeerId: inputPin.links[0] });
      }

      let changed = false;
      const next = prev.map((node) => {
        let nodeChanged = false;

        // 检查是否是需要清理旧连接的节点
        toCleanup.forEach(({ pinId, oldPeerId }) => {
          if (node.id === pinNodeIdIndexRef.current.get(oldPeerId)) {
            const newNode = nodeChanged ? (node as any) : node.clone();
            if (removePinLink(newNode, oldPeerId, pinId)) {
              nodeChanged = true;
              node = newNode;
            }
          }
        });

        // 检查是否是需要建立新连接的节点
        if (node.id === outputNodeId || node.id === inputNodeId) {
          const newNode = nodeChanged ? (node as any) : node.clone();
          const a = updatePinLink(newNode, outputPin.id, inputPin.id);
          const b = updatePinLink(newNode, inputPin.id, outputPin.id);
          if (a || b) {
            nodeChanged = true;
            node = newNode;
          }
        }

        if (nodeChanged) changed = true;
        return node;
      });
      return changed ? next : prev;
    });
  }, [saveHistory]);

  useEffect(() => {
    let lastWidth = window.innerWidth, lastHeight = window.innerHeight;
    const handleResize = () => {
      const dw = window.innerWidth - lastWidth, dh = window.innerHeight - lastHeight;
      setCanvas(prev => ({ ...prev, x: prev.x + dw / 2, y: prev.y + dh / 2 }));
      lastWidth = window.innerWidth; lastHeight = window.innerHeight;
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  const addVariable = useCallback((name: string, type: string, isGlobal: boolean = false) => {
    saveHistory();
    const id = `var-${crypto.randomUUID()}`;
    const newVar = { name, type, value: type === "int" ? 0 : type === "bool" ? false : type === "float" ? 0.0 : "" };
    if (isGlobal) {
      setGlobalVariables(prev => ({ ...prev, [id]: newVar }));
    } else {
      setVariables(prev => ({ ...prev, [id]: newVar }));
    }
    setSelectedVariableId(id);
  }, [saveHistory]);

  const updateVariable = useCallback((id: string, data: Partial<{ name: string; type: string; value: any }>) => {
    saveHistory();
    const isGlobal = !!globalVariablesRef.current[id];
    if (isGlobal) {
      setGlobalVariables(prev => ({ ...prev, [id]: { ...prev[id], ...data } }));
    } else {
      setVariables(prev => ({ ...prev, [id]: { ...prev[id], ...data } }));
    }

    if (data.name || data.type) {
      setNodes(prevNodes => {
        let nodesChanged = false;
        const nextNodes = prevNodes.map(node => {
          if (node.variableId === id) {
            nodesChanged = true;
            const newNode = node.clone();
            if (data.name) {
              const prefix = node.type === "get_variable" ? "Get" : "Set";
              newNode.title = `${prefix} ${data.name}`;
            }
            if (data.type) {
              const updatePins = (pins: Pin[]) => { pins.forEach(p => { if (p.type !== "exec") p.type = data.type as any; }); };
              updatePins(newNode.inputs); updatePins(newNode.outputs);
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
    saveHistory();
    const isGlobal = !!globalVariablesRef.current[id];
    if (isGlobal) {
      setGlobalVariables(prev => { const next = { ...prev }; delete next[id]; return next; });
    } else {
      setVariables(prev => { const next = { ...prev }; delete next[id]; return next; });
    }
    setNodes(prevNodes => {
      const pinsToDelete = new Set<string>();
      prevNodes.forEach((node) => { if (node.variableId === id) { node.inputs.forEach((p) => pinsToDelete.add(p.id)); node.outputs.forEach((p) => pinsToDelete.add(p.id)); } });
      return prevNodes.filter((node) => node.variableId !== id).map((node) => {
        let nodeChanged = false; const newNode = node.clone();
        const cleanPins = (pins: Pin[]) => { pins.forEach(pin => { const originalLength = pin.links.length; pin.links = pin.links.filter(linkId => !pinsToDelete.has(linkId)); if (pin.links.length !== originalLength) nodeChanged = true; }); };
        cleanPins(newNode.inputs); cleanPins(newNode.outputs);
        return nodeChanged ? newNode : node;
      });
    });
  }, []);

  const promoteVariable = useCallback((id: string) => {
    const varToPromote = variablesRef.current[id];
    if (!varToPromote) return;
    saveHistory();
    // Move from local to global
    setVariables(prev => { const next = { ...prev }; delete next[id]; return next; });
    setGlobalVariables(prev => ({ ...prev, [id]: varToPromote }));
    showToast(`变量 '${varToPromote.name}' 已提升为全局变量`, "success", 2000);
  }, [saveHistory, showToast]);

  const demoteVariable = useCallback((id: string) => {
    const varToDemote = globalVariablesRef.current[id];
    if (!varToDemote) return;
    saveHistory();
    // Move from global to local
    setGlobalVariables(prev => { const next = { ...prev }; delete next[id]; return next; });
    setVariables(prev => ({ ...prev, [id]: varToDemote }));
    showToast(`变量 '${varToDemote.name}' 已转为局部变量`, "success", 2000);
  }, [saveHistory, showToast]);

  const addFunction = useCallback((name: string) => {
    saveHistory();
    const id = `func-${crypto.randomUUID()}`;
    setFunctions(prev => ({ ...prev, [id]: { name } }));
  }, [saveHistory]);

  const deleteFunction = useCallback((id: string) => {
    saveHistory();
    setFunctions(prev => { const next = { ...prev }; delete next[id]; return next; });
  }, [saveHistory]);

  const addMacro = useCallback((name: string) => {
    saveHistory();
    const id = `macro-${crypto.randomUUID()}`;
    setMacros(prev => ({ ...prev, [id]: { name } }));
  }, [saveHistory]);

  const deleteMacro = useCallback((id: string) => {
    saveHistory();
    setMacros(prev => { const next = { ...prev }; delete next[id]; return next; });
  }, [saveHistory]);

  const saveGraphAs = useCallback(async () => {
    try {
      const data = serializeGraph(nodesRef.current, canvas, variablesRef.current, globalVariablesRef.current, functionsRef.current, macrosRef.current);
      const path = await save({ filters: [{ name: "JSON", extensions: ["json"] }], defaultPath: currentPath || `yssbi-graph-${Date.now()}.json` });
      if (path) {
        await writeTextFile(path, JSON.stringify(data, null, 2));
        const filename = path.split(/[\\/]/).pop() || "Untitled";
        setCurrentPath(path);
        setTabs(prev => prev.map(t => t.id === activeTabId ? { ...t, title: filename, path, isDirty: false } : t));
        showToast("已另存为", "success", 2000);
      }
    } catch (e) {
      console.error("Save As failed:", e);
      showToast(`另存为失败: ${e}`, "error", 5000);
    }
  }, [canvas, currentPath, activeTabId, showToast]);

  const saveGraph = useCallback(async () => {
    if (!currentPath) return saveGraphAs();
    try {
      const data = serializeGraph(nodesRef.current, canvas, variablesRef.current, globalVariablesRef.current, functionsRef.current, macrosRef.current);
      await writeTextFile(currentPath, JSON.stringify(data, null, 2));
      markDirty(false);
      showToast("已保存", "success", 2000);
    } catch (e) {
      console.error("Save failed:", e); saveGraphAs();
    }
  }, [canvas, currentPath, saveGraphAs, showToast]);

  const importGraph = useCallback(async (json?: string) => {
    try {
      let content = json; let path: string | null = null;
      if (!content) {
        const selected = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
        if (!selected || Array.isArray(selected)) return;
        path = selected as string; content = await readTextFile(path);
      }
      if (!content) return;
      const filename = path ? (path.split(/[\\/]/).pop() || "Untitled") : "Imported Graph";
      const { nodes: newNodes, canvas: newCanvas, localVariables: newVars, globalVariables: newGlobalVars, functions: newFuncs, macros: newMacros } = deserializeGraph(JSON.parse(content));

      // We merge global variables from the file into our current session's global variables
      setGlobalVariables(prev => ({ ...prev, ...newGlobalVars }));
      setFunctions(prev => ({ ...prev, ...newFuncs }));
      setMacros(prev => ({ ...prev, ...newMacros }));

      addTab(filename, {
        nodes: newNodes, canvas: newCanvas, variables: newVars || {}, history: { past: [], future: [] }, currentPath: path
      });
      showToast("已打开文件", "success", 2000);
    } catch (e) {
      console.error("Import failed:", e); showToast(`打开失败: ${e}`, "error", 5000);
    }
  }, [addTab, showToast]);

  const executeGraph = useCallback(async () => {
    try {
      showToast("开始执行...", "info", 1000);
      const data = serializeGraph(nodesRef.current, canvas, variablesRef.current, globalVariablesRef.current, functionsRef.current, macrosRef.current);
      const result = await invoke<string[]>("execute_graph", { data });
      result.forEach(log => {
        if (log.includes("[NODE PRINT]")) showToast(log, "success", 5000);
        else if (log.includes("[Error]")) showToast(log, "error", 5000);
      });
      if (result.length > 0) {
        const lastLog = result[result.length - 1];
        if (!lastLog.includes("[Error]")) showToast("执行完成", "success", 2000);
      }
    } catch (e) {
      console.error("Execution failed:", e); showToast(`执行失败: ${e}`, "error", 5000);
    }
  }, [canvas, showToast]);

  const [gesture, setGesture] = useState<Gesture>(null);
  const gestureRef = useRef<Gesture>(null);
  useEffect(() => { gestureRef.current = gesture; }, [gesture]);

  const [pendingConnection, setPendingConnection] = useState<Pin | null>(null);
  const [selection, setSelection] = useState<{ startX: number; startY: number; currentX: number; currentY: number } | null>(null);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; visible: boolean } | null>(null);

  // 处理重连和断开逻辑
  useEffect(() => {
    if (gesture?.type === "connect" && gesture.isReconnect) {
      const sourcePin = gesture.startPin;
      if (sourcePin.links.length > 0) {
        const peerId = sourcePin.links[sourcePin.links.length - 1];
        const peerPin = pinIndexRef.current.get(peerId);
        if (peerPin) {
          saveHistory();
          const sourceNodeId = pinNodeIdIndexRef.current.get(sourcePin.id);
          const peerNodeId = pinNodeIdIndexRef.current.get(peerPin.id);
          setNodes(prev => prev.map(n => {
            if (n.id === sourceNodeId || n.id === peerNodeId) {
              const newNode = n.clone();
              removePinLink(newNode, n.id === sourceNodeId ? sourcePin.id : peerPin.id, n.id === sourceNodeId ? peerPin.id : sourcePin.id);
              return newNode;
            }
            return n;
          }));
          setGesture({ ...gesture, startPin: peerPin, isReconnect: false });
        }
      }
    } else if (gesture?.type === "disconnect") {
      saveHistory();
      const targetPin = gesture.pin;
      const peerIds = [...targetPin.links];
      const targetNodeId = pinNodeIdIndexRef.current.get(targetPin.id);
      setNodes(prev => prev.map(n => {
        let changed = false;
        const newNode = (n.id === targetNodeId || peerIds.some(id => pinNodeIdIndexRef.current.get(id) === n.id)) ? n.clone() : n;
        if (n.id === targetNodeId) {
          const p = [...newNode.inputs, ...newNode.outputs].find(p => p.id === targetPin.id);
          if (p && p.links.length > 0) { p.links = []; changed = true; }
        }
        peerIds.forEach(peerId => {
          if (pinNodeIdIndexRef.current.get(peerId) === n.id) {
            if (removePinLink(newNode, peerId, targetPin.id)) changed = true;
          }
        });
        return changed ? newNode : n;
      }));
      setGesture(null);
    }
  }, [gesture, saveHistory]);

  const onCanvasWheel = useCallback((e: React.WheelEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest(".menubar-container") || target.closest(".sidebar-container") || target.closest(".menu-container") || target.closest(".hud-container")) return;
    e.preventDefault();
    const factor = 0.001;
    const nextScale = clamp(canvas.scale * (1 - e.deltaY * factor), 0.2, 4);
    const rect = e.currentTarget.getBoundingClientRect();
    const mouseX = e.clientX - rect.left, mouseY = e.clientY - rect.top;
    const worldX = (mouseX - canvas.x) / canvas.scale, worldY = (mouseY - canvas.y) / canvas.scale;
    setCanvas({ scale: nextScale, x: mouseX - worldX * nextScale, y: mouseY - worldY * nextScale });
  }, [canvas]);

  const onCanvasPointerMove = useCallback((e: PointerEvent) => {
    const currentGesture = gestureRef.current;
    if (!currentGesture) return;
    if (currentGesture.type === "pan") {
      const dx = e.clientX - currentGesture.lastX, dy = e.clientY - currentGesture.lastY;
      if (Math.abs(dx) > 2 || Math.abs(dy) > 2) currentGesture.moved = true;
      setCanvas((prev) => ({ ...prev, x: prev.x + dx, y: prev.y + dy }));
      currentGesture.lastX = e.clientX; currentGesture.lastY = e.clientY;
      setGesture({ ...currentGesture });
    } else if (currentGesture.type === "select") {
      currentGesture.currentX = e.clientX; currentGesture.currentY = e.clientY;
      setSelection({ ...currentGesture }); setGesture({ ...currentGesture });
    } else if (currentGesture.type === "connect") {
      currentGesture.currentX = e.clientX; currentGesture.currentY = e.clientY;
      setGesture({ ...currentGesture });
    }
  }, []);

  const onCanvasPointerUp = useCallback((e: PointerEvent) => {
    const currentGesture = gestureRef.current;

    if (currentGesture?.type === "select") {
      setSelection(null);
    } else if (currentGesture?.type === "pan" && !currentGesture.moved && e.button === 2) {
      setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
    } else if (currentGesture?.type === "connect") {
      const elements = document.elementsFromPoint(e.clientX, e.clientY);
      const pinEl = elements.find(el => el.closest("[data-pin-id]"))?.closest("[data-pin-id]");
      const targetPinId = pinEl?.getAttribute("data-pin-id");
      const sourcePin = currentGesture.startPin;

      if (!targetPinId) {
        setPendingConnection(sourcePin);
        setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
      } else if (targetPinId !== sourcePin.id) {
        connectPins(sourcePin.id, targetPinId);
      }
    }

    gestureRef.current = null;
    setGesture(null);
    window.removeEventListener("pointermove", onCanvasPointerMove);
    window.removeEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove, connectPins]);

  const onPinPointerDown = useCallback((e: React.PointerEvent, pin: Pin) => {
    e.stopPropagation(); e.preventDefault();
    setPendingConnection(null);
    setContextMenu(null);
    if (e.altKey) { setGesture({ type: "disconnect", pin }); return; }
    const start = { type: "connect" as const, startPin: pin, startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY, isReconnect: e.ctrlKey };
    setGesture(start);
    window.addEventListener("pointermove", onCanvasPointerMove);
    window.addEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove, onCanvasPointerUp]);

  const onCanvasPointerDown = useCallback((e: React.PointerEvent) => {
    setSelectedVariableId(null);
    setPendingConnection(null);
    setContextMenu(null);
    if (e.button === 0) {
      const start = { type: "select" as const, startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY };
      setSelection(start); setGesture(start);
    } else if (e.button === 1 || e.button === 2) {
      const start = { type: "pan" as const, lastX: e.clientX, lastY: e.clientY, moved: false };
      setGesture(start);
    }
    window.addEventListener("pointermove", onCanvasPointerMove);
    window.addEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove, onCanvasPointerUp]);

  return (
    <CanvasContext.Provider
      value={{
        canvas, setCanvas, nodes, setNodes, onCanvasWheel, onCanvasPointerDown, onPinPointerDown, selection, gesture, setGesture, contextMenu, setContextMenu, saveGraphAs, saveGraph, importGraph, executeGraph, variables, globalVariables, selectedVariableId, setSelectedVariableId, addVariable, updateVariable, deleteVariable, promoteVariable, demoteVariable,
        functions, addFunction, deleteFunction,
        macros, addMacro, deleteMacro,
        undo, redo, copy, paste, cut, deleteSelected, canUndo: history.past.length > 0, canRedo: history.future.length > 0, saveHistory, connectPins,
        tabs, activeTabId, setActiveTabId: handleSetActiveTabId, addTab, closeTab,
        pendingConnection, setPendingConnection
      }}
    >
      {children}
    </CanvasContext.Provider>
  );
};
