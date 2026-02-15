import { useCallback, useRef } from 'react';
import { Graph } from '@/shared/types/domain';
import { useProjectStore } from '@/features/core/project';
import { GraphService } from '@/services/graph/graphService';

// 辅助函数：生成唯一名称
const getUniqueName = (baseName: string, items: Record<string, { name: string }>) => {
  const names = Object.values(items).map(i => i.name);
  let name = baseName;
  let counter = 1;
  while (names.includes(name)) {
    name = `${baseName}_${counter}`;
    counter++;
  }
  return name;
};

/**
 * Graph Management Hook
 * 
 * 负责 Event/Function/Macro 的创建、更新、删除逻辑
 * - 生成唯一名称
 * - 调用 GraphService 与后端通信（后端会创建完整的 Graph 结构）
 * - 后端通过事件系统通知前端，由 projectSync 更新状态
 * - 通过 pendingActions 跟踪待处理的操作，在事件到达时执行
 */
export function useGraphManagement(
  openGraph: (id: string, name: string, type: any, data?: any) => void,
  closeTab: (id: string) => void,
  switchSidebarTab: (tab: 'events' | 'functions' | 'macros' | 'variables') => void
) {
  // 使用 ref 存储待处理的操作（创建后需要打开的 graph）
  const pendingActionsRef = useRef<{
    events: Map<string, () => void>;
    functions: Map<string, () => void>;
    macros: Map<string, () => void>;
  }>({
    events: new Map(),
    functions: new Map(),
    macros: new Map(),
  });

  // Events
  const addEvent = useCallback(async (name?: string) => {
    console.log("[useGraphManagement] addEvent called with name:", name);
    
    const store = useProjectStore.getState();
    // 从 graphs 中筛选出 events
    const events: Record<string, Graph> = {};
    for (const [id, graph] of Object.entries(store.graphs)) {
      if (graph.type === 'event') events[id] = graph;
    }
    
    const finalName = getUniqueName(name || "New Event", events);
    
    console.log("[useGraphManagement] Creating event:", { name: finalName });
    
    try {
      // 注册待处理操作：当后端事件到达时打开这个 event
      pendingActionsRef.current.events.set(finalName, () => {
        const updatedStore = useProjectStore.getState();
        const newEvent = Object.entries(updatedStore.graphs).find(
          ([_, graph]) => graph.type === 'event' && graph.name === finalName
        );
        
        if (newEvent) {
          const [id, event] = newEvent;
          console.log("[useGraphManagement] Opening newly created event:", id);
          openGraph(id, event.name, "event", event);
        }
      });
      
      // 调用后端 API 创建 Event
      await GraphService.createEvent(finalName);
      
      console.log("[useGraphManagement] Event creation request sent");
      
      // 切换到 events 标签页
      switchSidebarTab('events');
      
    } catch (error) {
      console.error("[useGraphManagement] Failed to create event:", error);
      // 清除待处理操作
      pendingActionsRef.current.events.delete(finalName);
      throw error;
    }
  }, [openGraph, switchSidebarTab]);

  // 处理 Event 创建事件的回调
  const handleEventCreated = useCallback((id: string, data: any) => {
    console.log("[useGraphManagement] handleEventCreated:", id, data);
    const action = pendingActionsRef.current.events.get(data.name);
    if (action) {
      action();
      pendingActionsRef.current.events.delete(data.name);
    }
  }, []);

  const updateEvent = useCallback(async (id: string, data: Partial<Graph>) => {
    const store = useProjectStore.getState();
    const currentGraph = store.graphs[id];
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateEvent(id, fullData);
      store.updateGraph(id, data);
    } catch (error) {
      console.error("[useGraphManagement] Failed to update event:", error);
      throw error;
    }
  }, []);

  const deleteEvent = useCallback(async (id: string) => {
    try {
      await GraphService.removeGraph(id);
      useProjectStore.getState().deleteGraph(id);
      closeTab(id);
    } catch (error) {
      console.error("[useGraphManagement] Failed to delete event:", error);
      throw error;
    }
  }, [closeTab]);

  // Functions
  const addFunction = useCallback(async (name?: string) => {
    console.log("[useGraphManagement] addFunction called with name:", name);
    
    const store = useProjectStore.getState();
    // 从 graphs 中筛选出 functions
    const functions: Record<string, Graph> = {};
    for (const [id, graph] of Object.entries(store.graphs)) {
      if (graph.type === 'function') functions[id] = graph;
    }
    
    const finalName = getUniqueName(name || "New Function", functions);
    
    console.log("[useGraphManagement] Creating function:", { name: finalName });
    
    try {
      // 注册待处理操作
      pendingActionsRef.current.functions.set(finalName, () => {
        const updatedStore = useProjectStore.getState();
        const newFunction = Object.entries(updatedStore.graphs).find(
          ([_, graph]) => graph.type === 'function' && graph.name === finalName
        );
        
        if (newFunction) {
          const [id, func] = newFunction;
          console.log("[useGraphManagement] Opening newly created function:", id);
          openGraph(id, func.name, "function", func);
        }
      });
      
      // 调用后端 API
      await GraphService.createFunction(finalName);
      
      console.log("[useGraphManagement] Function creation request sent");
      
      // 切换到 functions 标签页
      switchSidebarTab('functions');
      
    } catch (error) {
      console.error("[useGraphManagement] Failed to create function:", error);
      pendingActionsRef.current.functions.delete(finalName);
      throw error;
    }
  }, [openGraph, switchSidebarTab]);

  // 处理 Function 创建事件的回调
  const handleFunctionCreated = useCallback((id: string, data: any) => {
    console.log("[useGraphManagement] handleFunctionCreated:", id, data);
    const action = pendingActionsRef.current.functions.get(data.name);
    if (action) {
      action();
      pendingActionsRef.current.functions.delete(data.name);
    }
  }, []);

  const updateFunction = useCallback(async (id: string, data: Partial<Graph>) => {
    const store = useProjectStore.getState();
    const currentGraph = store.graphs[id];
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateFunction(id, fullData);
      store.updateGraph(id, data);
    } catch (error) {
      console.error("[useGraphManagement] Failed to update function:", error);
      throw error;
    }
  }, []);

  const deleteFunction = useCallback(async (id: string) => {
    try {
      await GraphService.removeGraph(id);
      useProjectStore.getState().deleteGraph(id);
      closeTab(id);
    } catch (error) {
      console.error("[useGraphManagement] Failed to delete function:", error);
      throw error;
    }
  }, [closeTab]);

  // Macros
  const addMacro = useCallback(async (name?: string) => {
    console.log("[useGraphManagement] addMacro called with name:", name);
    
    const store = useProjectStore.getState();
    // 从 graphs 中筛选出 macros
    const macros: Record<string, Graph> = {};
    for (const [id, graph] of Object.entries(store.graphs)) {
      if (graph.type === 'macro') macros[id] = graph;
    }
    
    const finalName = getUniqueName(name || "New Macro", macros);
    
    console.log("[useGraphManagement] Creating macro:", { name: finalName });
    
    try {
      // 注册待处理操作
      pendingActionsRef.current.macros.set(finalName, () => {
        const updatedStore = useProjectStore.getState();
        const newMacro = Object.entries(updatedStore.graphs).find(
          ([_, graph]) => graph.type === 'macro' && graph.name === finalName
        );
        
        if (newMacro) {
          const [id, macro] = newMacro;
          console.log("[useGraphManagement] Opening newly created macro:", id);
          openGraph(id, macro.name, "macro", macro);
        }
      });
      
      // 调用后端 API
      await GraphService.createMacro(finalName);
      
      console.log("[useGraphManagement] Macro creation request sent");
      
      // 切换到 macros 标签页
      switchSidebarTab('macros');
      
    } catch (error) {
      console.error("[useGraphManagement] Failed to create macro:", error);
      pendingActionsRef.current.macros.delete(finalName);
      throw error;
    }
  }, [openGraph, switchSidebarTab]);

  // 处理 Macro 创建事件的回调
  const handleMacroCreated = useCallback((id: string, data: any) => {
    console.log("[useGraphManagement] handleMacroCreated:", id, data);
    const action = pendingActionsRef.current.macros.get(data.name);
    if (action) {
      action();
      pendingActionsRef.current.macros.delete(data.name);
    }
  }, []);

  const updateMacro = useCallback(async (id: string, data: Partial<Graph>) => {
    const store = useProjectStore.getState();
    const currentGraph = store.graphs[id];
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateMacro(id, fullData);
      store.updateGraph(id, data);
    } catch (error) {
      console.error("[useGraphManagement] Failed to update macro:", error);
      throw error;
    }
  }, []);

  const deleteMacro = useCallback(async (id: string) => {
    try {
      await GraphService.removeGraph(id);
      useProjectStore.getState().deleteGraph(id);
      closeTab(id);
    } catch (error) {
      console.error("[useGraphManagement] Failed to delete macro:", error);
      throw error;
    }
  }, [closeTab]);

  return {
    // Events
    addEvent,
    updateEvent,
    deleteEvent,
    handleEventCreated,

    // Functions
    addFunction,
    updateFunction,
    deleteFunction,
    handleFunctionCreated,

    // Macros
    addMacro,
    updateMacro,
    deleteMacro,
    handleMacroCreated,
  };
}
