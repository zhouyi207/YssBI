import React, { useRef, useState, useCallback, useEffect, useMemo } from "react";
import { CanvasContext } from "./CanvasContext";
import { CanvasState, Gesture, Tab, SubGraphData, PinDefinition } from "../Types/canvas";
import { clamp } from "../../../types";
import { Pin, BaseNode } from "../Types/nodes";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import { invoke } from "@tauri-apps/api/core";
import { serializeSubGraph, deserializeSubGraph, serializeProject, deserializeProject } from "../Utils/io";
import { useUI } from "./UIProvider";
import { NODE_REGISTRY } from "../Nodes/registry";
import { createInternalNode, syncInternalNodePins } from "../Utils/internalNodes";

/* ================= Helper Functions ================= */

import { isCompatiblePins, isSingleLinkPin } from "../Utils/pinUtils";

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



export const CanvasProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const { showToast } = useUI();

  // --- Project Metadata ---
  const [currentPath, setCurrentPath] = useState<string | null>(null);


  // --- Collection State (The "Project") ---
  const [events, setEvents] = useState<Record<string, SubGraphData>>({});
  const [functions, setFunctions] = useState<Record<string, SubGraphData>>({});
  const [macros, setMacros] = useState<Record<string, SubGraphData>>({});

  const [globalVariables, setGlobalVariables] = useState<Record<string, { name: string; type: string; value: any }>>({});
  const eventsRef = useRef(events); useEffect(() => { eventsRef.current = events; }, [events]);
  const functionsRef = useRef(functions); useEffect(() => { functionsRef.current = functions; }, [functions]);
  const macrosRef = useRef(macros); useEffect(() => { macrosRef.current = macros; }, [macros]);
  const globalVariablesRef = useRef(globalVariables); useEffect(() => { globalVariablesRef.current = globalVariables; }, [globalVariables]);

  // --- Runtime Editor State ---
  const [nodes, setNodes] = useState<BaseNode[]>([]);
  const nodesRef = useRef(nodes); useEffect(() => { nodesRef.current = nodes; }, [nodes]);

  const [canvas, setCanvas] = useState<CanvasState>({ x: 0, y: 0, scale: 1 });
  const [variables, setVariables] = useState<Record<string, { name: string; type: string; value: any }>>({});
  const variablesRef = useRef(variables); useEffect(() => { variablesRef.current = variables; }, [variables]);

  const [history, setHistory] = useState<{ past: any[], future: any[] }>({ past: [], future: [] });

  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [selectedItemType, setSelectedItemType] = useState<'variable' | 'event' | 'function' | 'macro' | null>(null);

  const setSelectedInfo = useCallback((id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | null) => {
    setSelectedItemId(id);
    setSelectedItemType(type);
  }, []);

  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const activeTabIdRef = useRef(activeTabId); useEffect(() => { activeTabIdRef.current = activeTabId; }, [activeTabId]);


  // --- Initial Setup ---
  useEffect(() => {
    // Only initialize if we don't have anything yet
    if (Object.keys(eventsRef.current).length === 0 && tabs.length === 0) {
      const id = "default-event";
      const name = "Event Graph";
      const type = "event";
      const nodes = [
        createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", "On Run", "Internal", { x: 100, y: 100 }, [], [{ name: "Exec", type: "exec" }])
      ];
      const sub: SubGraphData = { id, name, type, nodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
      setEvents({ [id]: sub });
      setTabs([{ id, title: name, type }]);
      setActiveTabId(id);
      setSelectedInfo(id, type);
      setNodes(nodes);
    }
  }, []);

  // --- Syncing ---
  const syncActiveToCollection = useCallback(() => {
    const id = activeTabIdRef.current;
    if (!id) return;
    const tab = tabs.find(t => t.id === id);
    const existing = eventsRef.current[id] || functionsRef.current[id] || macrosRef.current[id];
    const subGraph = serializeSubGraph(
      id,
      tab?.title || "Untitled",
      tab?.type as any || "event",
      nodesRef.current,
      canvas,
      variablesRef.current,
      existing?.inputs || [],
      existing?.outputs || []
    );
    if (eventsRef.current[id]) setEvents(prev => ({ ...prev, [id]: subGraph }));
    else if (functionsRef.current[id]) setFunctions(prev => ({ ...prev, [id]: subGraph }));
    else if (macrosRef.current[id]) setMacros(prev => ({ ...prev, [id]: subGraph }));
  }, [tabs, canvas]);

  const handleSetActiveTabId = useCallback((newId: string | null, forceType?: 'event' | 'function' | 'macro', initialData?: SubGraphData) => {
    if (newId === activeTabId) return;

    syncActiveToCollection();
    if (!newId) {
      setNodes([]); setCanvas({ x: 0, y: 0, scale: 1 }); setVariables({}); setHistory({ past: [], future: [] });


    } else {
      const id = newId!;
      const source = initialData || eventsRef.current[id] || functionsRef.current[id] || macrosRef.current[id];
      const type = forceType || source?.type;
      if (source) {
        const { nodes: n, canvas: c, variables: v } = deserializeSubGraph(source);
        setNodes(n); setCanvas(c); setVariables(v); setHistory({ past: [], future: [] });
      } else {
        setNodes([]); setCanvas({ x: 0, y: 0, scale: 1 }); setVariables({}); setHistory({ past: [], future: [] });
      }
      if (type) setSelectedInfo(id, type);
    }
    setActiveTabId(newId);
  }, [activeTabId, syncActiveToCollection, setSelectedInfo]);

  const openSubGraph = useCallback((id: string, name: string, type: "event" | "function" | "macro", initialData?: SubGraphData) => {
    setTabs(prev => {
      if (prev.find(t => t.id === id)) return prev;
      return [...prev, { id, title: name, type }];
    });
    handleSetActiveTabId(id, type, initialData);
  }, [handleSetActiveTabId]);

  const closeTab = useCallback((id: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    setTabs(prev => {

      const index = prev.findIndex(t => t.id === id);
      const newTabs = prev.filter(t => t.id !== id);

      if (newTabs.length === 0) setTimeout(() => handleSetActiveTabId(null), 0);
      else if (id === activeTabId) {
        const nextId = newTabs[Math.min(index, newTabs.length - 1)].id;

        setTimeout(() => handleSetActiveTabId(nextId), 0);
      }
      return newTabs;
    });
  }, [activeTabId, handleSetActiveTabId]);

  // --- Actions ---
  const addFunction = useCallback((name: string) => {
    const id = `func-${crypto.randomUUID()}`;
    const nodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "function_entry", name, "Internal", { x: 50, y: 150 }, [], [{ name: "Then", type: "exec" }]),
      createInternalNode(`node-${crypto.randomUUID()}`, "function_return", "Return", "Internal", { x: 550, y: 150 }, [{ name: "In", type: "exec" }], [])
    ];
    const sub: SubGraphData = { id, name, type: "function", nodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    setFunctions(prev => ({ ...prev, [id]: sub }));
    openSubGraph(id, name, "function", sub);
  }, [openSubGraph]);

  const addEvent = useCallback((name: string) => {
    const id = `event-${crypto.randomUUID()}`;
    const nodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", name, "Internal", { x: 50, y: 150 }, [], [{ name: "Exec", type: "exec" }])
    ];
    const sub: SubGraphData = { id, name, type: "event", nodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    setEvents(prev => ({ ...prev, [id]: sub }));
    openSubGraph(id, name, "event", sub);
  }, [openSubGraph]);

  const addMacro = useCallback((name: string) => {
    const id = `macro-${crypto.randomUUID()}`;
    const nodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "macro_inputs", "Inputs", "Internal", { x: 50, y: 150 }, [], [{ name: "In", type: "exec" }]),
      createInternalNode(`node-${crypto.randomUUID()}`, "macro_outputs", "Outputs", "Internal", { x: 550, y: 150 }, [{ name: "Out", type: "exec" }], [])
    ];
    const sub: SubGraphData = { id, name, type: "macro", nodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    setMacros(prev => ({ ...prev, [id]: sub }));
    openSubGraph(id, name, "macro", sub);
  }, [openSubGraph]);

  const syncSubGraphInstanceNodes = useCallback((nodes: any[], subGraphId: string, inputs?: PinDefinition[], outputs?: PinDefinition[], name?: string) => {
    return nodes.map(n => {
      if (n.subGraphId !== subGraphId) return n;
      const newNode = (n instanceof BaseNode) ? n.clone() : Object.assign(Object.create(Object.getPrototypeOf(n)), n);

      // 同步 Title
      if (name) {
        newNode.title = name;
      }

      const synchronizePins = (newPinDefs: PinDefinition[], existingPins: Pin[], direction: 'input' | 'output') => {
        // Separate exec and data pins
        const execPins = existingPins.filter(p => p.type === 'exec');
        const dataPins = existingPins.filter(p => p.type !== 'exec');

        // Map new data pin definitions
        const newDataPins = newPinDefs.map(newDef => {
          const newPinId = `${newNode.id}_${direction === 'input' ? 'in' : 'out'}_${newDef.id}`;
          // Try to find existing pin by ID or by name+type to preserve connections
          const existingPin = dataPins.find(p => p.id === newPinId) ||
            dataPins.find(p => p.name === newDef.name && p.type === newDef.type);


          return {
            id: newPinId,
            nodeId: newNode.id,
            name: newDef.name,
            type: newDef.type as any,
            direction: direction,
            links: existingPin ? existingPin.links : []
          };
        });
        // Return exec pins first, then data pins
        return [...execPins, ...newDataPins];
      };

      if (inputs) newNode.inputs = synchronizePins(inputs, n.inputs, 'input');
      if (outputs) newNode.outputs = synchronizePins(outputs, n.outputs, 'output');

      return newNode;
    });
  }, []);

  const updateFunction = useCallback((id: string, data: Partial<SubGraphData>) => {
    setFunctions(prev => {
      const newCol = { ...prev };
      Object.keys(newCol).forEach(k => {
        newCol[k] = { ...newCol[k], nodes: syncSubGraphInstanceNodes(newCol[k].nodes, id, data.inputs, data.outputs, data.name) };
      });
      newCol[id] = { ...newCol[id], ...data };
      functionsRef.current = newCol;
      return newCol;
    });

    setMacros(prev => {
      const newCol = { ...prev };
      Object.keys(newCol).forEach(k => {
        newCol[k] = { ...newCol[k], nodes: syncSubGraphInstanceNodes(newCol[k].nodes, id, data.inputs, data.outputs, data.name) };
      });
      macrosRef.current = newCol;
      return newCol;
    });


    setEvents(prev => {
      const newCol = { ...prev };
      Object.keys(newCol).forEach(k => {
        newCol[k] = { ...newCol[k], nodes: syncSubGraphInstanceNodes(newCol[k].nodes, id, data.inputs, data.outputs, data.name) };
      });
      eventsRef.current = newCol;
      return newCol;
    });

    if (activeTabIdRef.current === id) {
      if (data.name && nodesRef.current.some(n => n.isInternal && (n.type === 'function_entry' || n.type === 'macro_inputs'))) {
        setNodes(nodes => nodes.map(n => {
          if (n.isInternal && (n.type === 'function_entry' || n.type === 'macro_inputs')) {
            const clone = n.clone();
            clone.title = data.name!;
            return clone;
          }
          return n;
        }));
      }
      if (data.inputs || data.outputs) {
        setNodes(nodes => {
          const updatedNodes = nodes.map(n => {
            if (!n.isInternal) return n;
            const clone = n.clone();
            if (n.type === "function_entry" && data.inputs) syncInternalNodePins(clone, data.inputs, true);
            if (n.type === "function_return" && data.outputs) syncInternalNodePins(clone, data.outputs, false);
            return clone;
          });


          // Clean up invalid links after pin sync
          const allPinIds = new Set<string>();
          updatedNodes.forEach(n => {
            n.inputs.forEach(p => allPinIds.add(p.id));
            n.outputs.forEach(p => allPinIds.add(p.id));

          });
          return updatedNodes.map(n => {
            const clone = n.clone();
            let changed = false;
            clone.inputs.forEach(p => {
              const validLinks = p.links.filter(linkId => allPinIds.has(linkId));
              if (validLinks.length !== p.links.length) {
                p.links = validLinks;
                changed = true;
              }
            });
            clone.outputs.forEach(p => {
              const validLinks = p.links.filter(linkId => allPinIds.has(linkId));
              if (validLinks.length !== p.links.length) {
                p.links = validLinks;
                changed = true;
              }
            });
            return changed ? clone : n;
          });
        });
      }
    } else {
      // Only sync instance nodes if we're not on the active tab
      setNodes(nodes => syncSubGraphInstanceNodes(nodes, id, data.inputs, data.outputs, data.name));
    }
  }, [syncSubGraphInstanceNodes]);



  const updateEvent = useCallback((id: string, data: Partial<SubGraphData>) => {
    setEvents(prev => {
      const next = { ...prev, [id]: { ...prev[id], ...data } };
      eventsRef.current = next;
      return next;
    });
    if (data.name) {
      setTabs(prev => prev.map(t => t.id === id ? { ...t, title: data.name! } : t));
    }
  }, []);
  const updateMacro = useCallback((id: string, data: Partial<SubGraphData>) => {
    setMacros(prev => {
      const newCol = { ...prev };
      Object.keys(newCol).forEach(k => {
        newCol[k] = { ...newCol[k], nodes: syncSubGraphInstanceNodes(newCol[k].nodes, id, data.inputs, data.outputs, data.name) };

      });
      newCol[id] = { ...newCol[id], ...data };
      macrosRef.current = newCol;
      return newCol;
    });

    if (data.name) {
      setTabs(prev => prev.map(t => t.id === id ? { ...t, title: data.name! } : t));
    }


    setFunctions(prev => {
      const newCol = { ...prev };
      Object.keys(newCol).forEach(k => {
        newCol[k] = { ...newCol[k], nodes: syncSubGraphInstanceNodes(newCol[k].nodes, id, data.inputs, data.outputs, data.name) };
      });
      functionsRef.current = newCol;
      return newCol;
    });
    setEvents(prev => {
      const newCol = { ...prev };
      Object.keys(newCol).forEach(k => {
        newCol[k] = { ...newCol[k], nodes: syncSubGraphInstanceNodes(newCol[k].nodes, id, data.inputs, data.outputs, data.name) };
      });
      eventsRef.current = newCol;
      return newCol;
    });


    if (activeTabIdRef.current === id) {
      if (data.name && nodesRef.current.some(n => n.isInternal && (n.type === 'macro_inputs'))) {
        setNodes(nodes => nodes.map(n => {
          if (n.isInternal && n.type === 'macro_inputs') {
            const clone = n.clone();
            clone.title = data.name!;
            return clone;
          }
          return n;
        }));
      }
      if (data.inputs || data.outputs) {
        setNodes(nodes => {
          const updatedNodes = nodes.map(n => {
            if (!n.isInternal) return n;
            const clone = n.clone();
            if (n.type === "macro_inputs" && data.inputs) syncInternalNodePins(clone, data.inputs, true);
            if (n.type === "macro_outputs" && data.outputs) syncInternalNodePins(clone, data.outputs, false);
            return clone;
          });
          // Clean up invalid links after pin sync
          const allPinIds = new Set<string>();
          updatedNodes.forEach(n => {
            n.inputs.forEach(p => allPinIds.add(p.id));
            n.outputs.forEach(p => allPinIds.add(p.id));
          });
          return updatedNodes.map(n => {
            const clone = n.clone();
            let changed = false;
            clone.inputs.forEach(p => {
              const validLinks = p.links.filter(linkId => allPinIds.has(linkId));
              if (validLinks.length !== p.links.length) {
                p.links = validLinks;
                changed = true;
              }
            });
            clone.outputs.forEach(p => {
              const validLinks = p.links.filter(linkId => allPinIds.has(linkId));
              if (validLinks.length !== p.links.length) {
                p.links = validLinks;
                changed = true;
              }
            });
            return changed ? clone : n;
          });
        });
      }
    } else {
      // Only sync instance nodes if we're not on the active tab
      setNodes(nodes => syncSubGraphInstanceNodes(nodes, id, data.inputs, data.outputs, data.name));
    }
  }, [syncSubGraphInstanceNodes]);

  const deleteFunction = useCallback((id: string) => {
    setFunctions(prev => { const n = { ...prev }; delete n[id]; return n; });
    closeTab(id);
  }, [closeTab]);
  const deleteEvent = useCallback((id: string) => {
    setEvents(prev => { const n = { ...prev }; delete n[id]; return n; });
    closeTab(id);
  }, [closeTab]);
  const deleteMacro = useCallback((id: string) => {
    setMacros(prev => { const n = { ...prev }; delete n[id]; return n; });
    closeTab(id);
  }, [closeTab]);



  const addVariable = useCallback((name: string, type: string, isGlobal: boolean = false) => {
    const id = `var-${crypto.randomUUID()}`;
    const newVar = { name, type, value: type === "int" ? 0 : type === "bool" ? false : type === "float" ? 0.0 : "" };
    if (isGlobal) setGlobalVariables(prev => ({ ...prev, [id]: newVar }));
    else setVariables(prev => ({ ...prev, [id]: newVar }));
    setSelectedInfo(id, 'variable');
  }, [setSelectedInfo]);


  const updateVariable = useCallback((id: string, data: Partial<{ name: string; type: string; value: any }>) => {
    const isGlobal = !!globalVariablesRef.current[id];
    const oldVar = isGlobal ? globalVariablesRef.current[id] : variablesRef.current[id];
    if (!oldVar) return;


    // 1. 更新变量状态和 Ref (确保同步)
    if (isGlobal) {
      setGlobalVariables(prev => {
        const next = { ...prev, [id]: { ...prev[id], ...data } };
        globalVariablesRef.current = next;
        return next;
      });
    } else {
      setVariables(prev => {
        const next = { ...prev, [id]: { ...prev[id], ...data } };
        variablesRef.current = next;
        return next;
      });
    }

    // 2. 如果名称发生变化，同步更新所有相关节点的 Title
    if (data.name !== undefined && data.name !== oldVar.name) {
      const updateNodeTitles = (nodes: BaseNode[]) => {
        return nodes.map(n => {
          if (n.variableId === id) {
            const newNode = n.clone();
            if (n.type === 'get_variable') newNode.title = `Get ${data.name}`;
            if (n.type === 'set_variable') newNode.title = `Set ${data.name}`;
            return newNode;
          }
          return n;
        });
      };

      // 更新当前激活图表的节点
      setNodes(prev => updateNodeTitles(prev));


      // 更新整个项目中所有子图的节点
      const syncInternalNodes = (col: Record<string, SubGraphData>) => {
        const newCol = { ...col };
        Object.keys(newCol).forEach(key => {
          newCol[key] = {
            ...newCol[key],
            nodes: updateNodeTitles(newCol[key].nodes as any[]) as any
          };
        });
        return newCol;
      };

      setEvents(prev => syncInternalNodes(prev));
      setFunctions(prev => syncInternalNodes(prev));
      setMacros(prev => syncInternalNodes(prev));
    }
  }, []);
  const deleteVariable = useCallback((id: string) => {
    if (globalVariablesRef.current[id]) setGlobalVariables(prev => { const n = { ...prev }; delete n[id]; return n; });
    else if (variablesRef.current[id]) setVariables(prev => { const n = { ...prev }; delete n[id]; return n; });
  }, []);
  const promoteVariable = useCallback((id: string) => {
    const v = variablesRef.current[id]; if (!v) return;
    setVariables(prev => {
      const n = { ...prev };
      delete n[id];
      variablesRef.current = n;
      return n;
    });
    setGlobalVariables(prev => {
      const n = { ...prev, [id]: v };
      globalVariablesRef.current = n;
      return n;
    });
  }, []);
  const demoteVariable = useCallback((id: string) => {
    const v = globalVariablesRef.current[id]; if (!v) return;
    setGlobalVariables(prev => {
      const n = { ...prev };
      delete n[id];
      globalVariablesRef.current = n;
      return n;
    });
    setVariables(prev => {
      const n = { ...prev, [id]: v };
      variablesRef.current = n;
      return n;
    });
  }, []);
  // --- Persistence ---
  const saveGraphAs = useCallback(async () => {
    try {


      syncActiveToCollection();
      const project = serializeProject(globalVariablesRef.current, eventsRef.current, functionsRef.current, macrosRef.current);
      const path = await save({ filters: [{ name: "JSON", extensions: ["json"] }] });
      if (path) {
        await writeTextFile(path, JSON.stringify(project, null, 2));
        setCurrentPath(path);
        showToast("项目已保存", "success", 2000);
      }
    } catch (e) { console.error(e); }
  }, [syncActiveToCollection, showToast]);

  const saveGraph = useCallback(async () => {
    if (!currentPath) return saveGraphAs();
    syncActiveToCollection();
    const project = serializeProject(globalVariablesRef.current, eventsRef.current, functionsRef.current, macrosRef.current);
    await writeTextFile(currentPath, JSON.stringify(project, null, 2));
    showToast("项目已保存", "success", 2000);
  }, [currentPath, saveGraphAs, syncActiveToCollection, showToast]);




  // --- Interaction (Gesture, Selection, Connect) ---
  const importGraph = useCallback(async (json?: string) => {
    try {
      let content = json; let path: string | null = null;
      if (!content) {
        const selected = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
        if (!selected || Array.isArray(selected)) return;
        path = selected as string; content = await readTextFile(path);
      }
      if (!content) return;
      const project = deserializeProject(content);
      setGlobalVariables(project.globalVariables); setEvents(project.events); setFunctions(project.functions); setMacros(project.macros);
      setCurrentPath(path);
      const first = Object.values(project.events)[0] || Object.values(project.functions)[0];
      if (first) {
        setTabs([{ id: first.id, title: first.name, type: first.type }]);
        setActiveTabId(first.id);
        const { nodes: n, canvas: c, variables: v } = deserializeSubGraph(first);
        setNodes(n); setCanvas(c); setVariables(v);
      }
    } catch (e) { console.error(e); }
  }, []);



  // --- Indices ---
  const pinIndex = useMemo(() => {
    const map = new Map<string, Pin>();
    nodes.forEach(n => { n.inputs.forEach(p => map.set(p.id, p)); n.outputs.forEach(p => map.set(p.id, p)); });
    return map;
  }, [nodes]);
  const pinNodeIdIndex = useMemo(() => {
    const map = new Map<string, string>();
    nodes.forEach(n => { n.inputs.forEach(p => map.set(p.id, n.id)); n.outputs.forEach(p => map.set(p.id, n.id)); });
    return map;
  }, [nodes]);
  const pinIndexRef = useRef(pinIndex); useEffect(() => { pinIndexRef.current = pinIndex; }, [pinIndex]);
  const pinNodeIdIndexRef = useRef(pinNodeIdIndex); useEffect(() => { pinNodeIdIndexRef.current = pinNodeIdIndex; }, [pinNodeIdIndex]);



  // --- Interaction (Gesture, Selection, Connect) ---
  const [gesture, setGesture] = useState<Gesture>(null);
  const gestureRef = useRef<Gesture>(null); useEffect(() => { gestureRef.current = gesture; }, [gesture]);
  const [pendingConnection, setPendingConnection] = useState<Pin | null>(null);
  const [selection, setSelection] = useState<{ startX: number, startY: number, currentX: number, currentY: number } | null>(null);
  const selectionRef = useRef(selection); useEffect(() => { selectionRef.current = selection; }, [selection]);
  const [contextMenu, setContextMenu] = useState<{ x: number, y: number, visible: boolean } | null>(null);

  const saveHistory = useCallback(() => {
    if (!activeTabIdRef.current) return;
    const existing = eventsRef.current[activeTabIdRef.current!] || functionsRef.current[activeTabIdRef.current!] || macrosRef.current[activeTabIdRef.current!];
    const serial = serializeSubGraph(
      "h", "h", "event", nodesRef.current, canvas, variablesRef.current,
      existing?.inputs || [], existing?.outputs || []
    );
    setHistory(prev => ({ past: [...prev.past, serial].slice(-50), future: [] }));
  }, [canvas]);

  const connectPins = useCallback((a: string, b: string) => {
    const pA = pinIndexRef.current.get(a); const pB = pinIndexRef.current.get(b);
    if (!pA || !pB || !isCompatiblePins(pA, pB)) return;
    saveHistory();

    // 收集需要清除的旧连接
    const oldLinksToRemove = new Set<string>();

    // 如果 pA 是单连接 pin 且已有连接，记录旧连接需要被清除
    if (isSingleLinkPin(pA) && pA.links.length > 0) {
      pA.links.forEach(linkId => oldLinksToRemove.add(linkId));
    }

    // 如果 pB 是单连接 pin 且已有连接，记录旧连接需要被清除
    if (isSingleLinkPin(pB) && pB.links.length > 0) {
      pB.links.forEach(linkId => oldLinksToRemove.add(linkId));
    }

    const nA = pinNodeIdIndexRef.current.get(a); const nB = pinNodeIdIndexRef.current.get(b);

    setNodes(prev => prev.map(n => {
      const newNode = n.clone();
      let changed = false;
      // 清除旧连接：遍历所有 pins，移除指向被替换连接的 links
      [...newNode.inputs, ...newNode.outputs].forEach(p => {
        // 如果这个 pin 的 links 中包含了需要被移除的连接
        if (oldLinksToRemove.has(p.id)) {
          // 移除指向 pA 或 pB 的连接
          const before = p.links.length;
          p.links = p.links.filter(linkId => linkId !== a && linkId !== b);
          if (p.links.length !== before) changed = true;
        }
      });


      // 建立新连接
      if (n.id === nA) {
        if (updatePinLink(newNode, a, b)) changed = true;
      }
      if (n.id === nB) {
        if (updatePinLink(newNode, b, a)) changed = true;
      }

      return changed ? newNode : n;
    }));
  }, [saveHistory]);


  const disconnectPin = useCallback((pinId: string) => {
    const pin = pinIndexRef.current.get(pinId);
    if (!pin || pin.links.length === 0) return;
    saveHistory();
    setNodes(prev => prev.map(n => {
      const newNode = n.clone();
      let changed = false;
      const process = (ps: Pin[]) => ps.forEach(p => {
        if (p.id === pinId) { p.links = []; changed = true; }
        else if (p.links.includes(pinId)) {
          p.links = p.links.filter(l => l !== pinId);
          changed = true;
        }

      });
      process(newNode.inputs); process(newNode.outputs);
      return changed ? newNode : n;
    }));
  }, [saveHistory]);


  const onCanvasPointerMove = useCallback((e: PointerEvent) => {
    const g = gestureRef.current; if (!g) return;
    if (g.type === "pan") {
      const dx = e.clientX - g.lastX, dy = e.clientY - g.lastY;
      setCanvas(prev => ({ ...prev, x: prev.x + dx, y: prev.y + dy }));
      g.lastX = e.clientX; g.lastY = e.clientY; g.moved = true;
      setGesture({ ...g });
    } else if (g.type === "select" || g.type === "connect") {
      g.currentX = e.clientX; g.currentY = e.clientY;
      if (g.type === "select") setSelection({ ...g });
      setGesture({ ...g });
    } else if (g.type === "drag") {
      const dx = (e.clientX - g.lastX) / canvas.scale;
      const dy = (e.clientY - g.lastY) / canvas.scale;
      if (Math.abs(dx) > 0.01 || Math.abs(dy) > 0.01) {
        g.moved = true;
        setNodes(prev => prev.map(n => {
          if (!n.selected) return n;
          const newNode = n.clone();
          newNode.position = { x: n.position.x + dx, y: n.position.y + dy };
          return newNode;
        }));
        g.lastX = e.clientX; g.lastY = e.clientY;
        setGesture({ ...g });
      }
    }
  }, [canvas.scale]);


  const onCanvasPointerUp = useCallback((e: PointerEvent) => {
    const g = gestureRef.current; if (!g) return;
    if (g.type === "select") {
      const rect = selectionRef.current;
      if (rect && (Math.abs(rect.startX - rect.currentX) > 5 || Math.abs(rect.startY - rect.currentY) > 5)) {
        const x1 = Math.min(rect.startX, rect.currentX), x2 = Math.max(rect.startX, rect.currentX);
        const y1 = Math.min(rect.startY, rect.currentY), y2 = Math.max(rect.startY, rect.currentY);
        setNodes(prev => prev.map(n => {
          const el = document.querySelector(`[data-node-id="${n.id}"]`);
          if (!el) return n;
          const r = el.getBoundingClientRect();
          const overlap = !(r.right < x1 || r.left > x2 || r.bottom < y1 || r.top > y2);
          const newNode = n.clone(); newNode.selected = overlap; return newNode;
        }));

      }
      setSelection(null);
    } else if (g.type === "drag") {
      if (g.moved) saveHistory();
    } else if (g.type === "connect") {
      const el = document.elementsFromPoint(e.clientX, e.clientY).find(x => x.closest("[data-pin-id]"))?.closest("[data-pin-id]");
      const targetId = el?.getAttribute("data-pin-id");
      if (targetId && targetId !== g.startPin.id) connectPins(g.startPin.id, targetId);
      else if (!targetId) { setPendingConnection(g.startPin); setContextMenu({ x: e.clientX, y: e.clientY, visible: true }); }
    } else if (g.type === "pan" && !g.moved && e.button === 2) {
      setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
    }
    setGesture(null);
    window.removeEventListener("pointermove", onCanvasPointerMove);
    window.removeEventListener("pointerup", onCanvasPointerUp);
  }, [connectPins, onCanvasPointerMove]);


  const onCanvasPointerDown = useCallback((e: React.PointerEvent) => {
    setPendingConnection(null); setContextMenu(null);
    let g: Gesture = null;
    if (e.button === 0) {
      setSelectedInfo(null, null);
      setNodes(prev => {
        if (!prev.some(n => n.selected)) return prev;
        return prev.map(n => {
          if (!n.selected) return n;
          const clone = n.clone(); clone.selected = false; return clone;
        });
      });
      g = { type: "select", startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY };
    }
    else if (e.button === 1 || e.button === 2) g = { type: "pan", lastX: e.clientX, lastY: e.clientY, moved: false };
    if (g) { setGesture(g); window.addEventListener("pointermove", onCanvasPointerMove); window.addEventListener("pointerup", onCanvasPointerUp); }
  }, [onCanvasPointerMove, onCanvasPointerUp, setSelectedInfo]);



  const onPinPointerDown = useCallback((e: React.PointerEvent, pin: Pin) => {
    e.stopPropagation(); e.preventDefault();
    if (e.altKey) {
      disconnectPin(pin.id);
      return;
    }
    const g: Gesture = { type: "connect", startPin: pin, startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY };
    setGesture(g); window.addEventListener("pointermove", onCanvasPointerMove); window.addEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove, onCanvasPointerUp, disconnectPin]);

  const onNodePointerDown = useCallback((e: React.PointerEvent, node: BaseNode) => {
    e.stopPropagation(); if (e.button !== 0) return;
    if (node.variableId) setSelectedInfo(node.variableId, 'variable');
    else setSelectedInfo(null, null);

    setNodes(prev => {
      const isSel = prev.find(n => n.id === node.id)?.selected;
      if (isSel && !e.shiftKey) return prev;
      return prev.map(n => {
        const newNode = n.clone();
        newNode.selected = n.id === node.id ? true : (e.shiftKey ? n.selected : false);
        return newNode;
      });
    });
    const g: Gesture = { type: "drag", nodeId: node.id, lastX: e.clientX, lastY: e.clientY, moved: false };
    setGesture(g); window.addEventListener("pointermove", onCanvasPointerMove); window.addEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove, onCanvasPointerUp, setSelectedInfo]);



  const onCanvasWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault(); const factor = 0.001; const nextScale = clamp(canvas.scale * (1 - e.deltaY * factor), 0.2, 4);
    const rect = e.currentTarget.getBoundingClientRect(); const mouseX = e.clientX - rect.left, mouseY = e.clientY - rect.top;
    const worldX = (mouseX - canvas.x) / canvas.scale, worldY = (mouseY - canvas.y) / canvas.scale;
    setCanvas({ scale: nextScale, x: mouseX - worldX * nextScale, y: mouseY - worldY * nextScale });
  }, [canvas]);


  const clipboardRef = useRef<BaseNode[]>([]);
  const deleteSelected = useCallback(() => {
    const selectedNodes = nodesRef.current.filter(n => n.selected && !n.isInternal);
    if (selectedNodes.length === 0) return;



    const deletedNodeIds = new Set(selectedNodes.map(n => n.id));
    const deletedPinIds = new Set<string>();
    selectedNodes.forEach(n => {
      n.inputs.forEach(p => deletedPinIds.add(p.id));
      n.outputs.forEach(p => deletedPinIds.add(p.id));
    });
    saveHistory();
    setNodes(prev => prev
      .filter(n => !deletedNodeIds.has(n.id))
      .map(n => {
        const newNode = n.clone();
        newNode.inputs.forEach(p => {
          p.links = p.links.filter(linkId => !deletedPinIds.has(linkId));

        });
        newNode.outputs.forEach(p => {
          p.links = p.links.filter(linkId => !deletedPinIds.has(linkId));
        });
        return newNode;
      })
    );



    // Clear selection if the selected item was deleted
    if (selectedItemId && (deletedNodeIds.has(selectedItemId) || deletedPinIds.has(selectedItemId))) {
      setSelectedInfo(null, null);
    }
  }, [saveHistory, selectedItemId, setSelectedInfo]);

  const executeGraph = useCallback(async () => {
    try {
      syncActiveToCollection();
      showToast("开始执行项目...", "info", 1000);
      const project = serializeProject(globalVariablesRef.current, eventsRef.current, functionsRef.current, macrosRef.current);
      const result = await invoke<string[]>("execute_graph", { data: project });
      result.forEach(log => {
        if (log.includes("[NODE PRINT]")) showToast(log, "success", 5000);
        else if (log.includes("[Error]")) showToast(log, "error", 5000);
      });
      showToast("系统执行完成", "info", 2000);
    } catch (e) {
      console.error(e); showToast(`执行失败: ${e}`, "error", 5000);
    }
  }, [syncActiveToCollection, showToast]);

  const undo = useCallback(() => {
    setHistory(prev => {
      if (prev.past.length === 0) return prev;
      const newPast = [...prev.past]; const prevState = newPast.pop()!;
      const { nodes: n, canvas: c, variables: v } = deserializeSubGraph(prevState);
      const curr = serializeSubGraph(activeTabIdRef.current!, "h", "event", nodesRef.current, canvas, variablesRef.current);
      setNodes(n); setCanvas(c); setVariables(v);
      return { past: newPast, future: [curr, ...prev.future] };
    });
  }, [canvas]);


  const redo = useCallback(() => {
    setHistory(prev => {
      if (prev.future.length === 0) return prev;
      const newFut = [...prev.future]; const nextState = newFut.shift()!;
      const { nodes: n, canvas: c, variables: v } = deserializeSubGraph(nextState);
      const curr = serializeSubGraph(activeTabIdRef.current!, "h", "event", nodesRef.current, canvas, variablesRef.current);
      setNodes(n); setCanvas(c); setVariables(v);
      return { past: [...prev.past, curr], future: newFut };
    });
  }, [canvas]);


  const paste = useCallback((pos?: { x: number; y: number }) => {
    if (clipboardRef.current.length === 0) return;
    saveHistory();
    const clipboard = clipboardRef.current.filter(n => NODE_REGISTRY.getDefinition(n.type));
    if (clipboard.length === 0) return;
    let targetX = pos ? pos.x : -canvas.x / canvas.scale + window.innerWidth / 4 / canvas.scale;
    let targetY = pos ? pos.y : -canvas.y / canvas.scale + window.innerHeight / 4 / canvas.scale;
    let minX = Math.min(...clipboard.map(n => n.position.x));
    let minY = Math.min(...clipboard.map(n => n.position.y));
    const offX = targetX - minX, offY = targetY - minY;
    const idMap = new Map<string, string>();
    const newNodes = clipboard.map(n => {
      const newNode = n.clone(); const newId = `node-${crypto.randomUUID()}`;
      newNode.id = newId; newNode.position = { x: n.position.x + offX, y: n.position.y + offY };
      newNode.selected = true;
      const updatePins = (ps: Pin[]) => ps.forEach(p => { const old = p.id; p.id = `${newId}-${crypto.randomUUID().slice(0, 8)}`; p.nodeId = newId; idMap.set(old, p.id); });
      updatePins(newNode.inputs); updatePins(newNode.outputs);
      return newNode;
    });
    newNodes.forEach(n => [...n.inputs, ...n.outputs].forEach(p => p.links = p.links.map(l => idMap.get(l)).filter((l): l is string => !!l)));
    setNodes(prev => [...prev.map(n => { const c = n.clone(); c.selected = false; return c; }), ...newNodes]);
  }, [canvas, saveHistory]);

  const copy = useCallback(() => {
    const sel = nodesRef.current.filter(n => n.selected && !n.isInternal);
    if (sel.length > 0) clipboardRef.current = sel.map(n => n.clone());
  }, []);
  const cut = useCallback(() => {
    copy();
    deleteSelected();
  }, [copy, deleteSelected]);
  return (
    <CanvasContext.Provider value={{
      canvas, setCanvas, nodes, setNodes, onCanvasWheel, onCanvasPointerDown, onNodePointerDown, onPinPointerDown, selection, gesture, setGesture, contextMenu, setContextMenu,
      saveGraphAs, saveGraph, importGraph, executeGraph, variables, globalVariables,
      selectedItemId, selectedItemType, setSelectedInfo,
      addVariable, updateVariable, deleteVariable, promoteVariable, demoteVariable,
      events, addEvent, updateEvent, deleteEvent,
      functions, addFunction, updateFunction, deleteFunction,
      macros, addMacro, updateMacro, deleteMacro,
      undo, redo, copy, paste, cut, deleteSelected, canUndo: history.past.length > 0, canRedo: history.future.length > 0, saveHistory, connectPins,
      tabs, activeTabId, setActiveTabId: handleSetActiveTabId, openSubGraph, addTab: () => addEvent("New Item"), closeTab, pendingConnection, setPendingConnection
    }}>
      {children}
    </CanvasContext.Provider>
  );
};
