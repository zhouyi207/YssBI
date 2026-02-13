import { useCallback } from 'react';
import { SubGraphData } from '@/views/EditorView/Types/canvas';
import { useProjectStore } from '@/features/project';
import { createInternalNode } from '@/views/EditorView/Utils/internalNodes';
import { uiStore } from '@/features/ui/UIStore';

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
 * SubGraph Management Hook
 * Handles creation, update, and deletion of events, functions, and macros
 */
export function useSubGraphManagement(
  openSubGraph: (id: string, name: string, type: any, data?: any) => void,
  closeTab: (id: string) => void,
  switchSidebarTab: (tab: 'events' | 'functions' | 'macros' | 'variables') => void
) {
  // Events
  const addEvent = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New Event", st.events);
    const id = `event-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(
        `node-${crypto.randomUUID()}`,
        "event_on_run",
        finalName,
        ["Internal"],
        { x: 50, y: 150 },
        [],
        [{ name: "Exec", type: "exec" }]
      )
    ];
    const sub: SubGraphData = {
      id,
      name: finalName,
      type: "event",
      nodes: tNodes,
      canvas: { x: 0, y: 0, scale: 1 },
      variables: {},
      inputs: [],
      outputs: []
    };
    st.addEvent(id, sub);
    openSubGraph(id, finalName, "event", sub);
    switchSidebarTab('events');
  }, [openSubGraph, switchSidebarTab]);

  const updateEvent = useCallback((id: string, data: Partial<SubGraphData>) => {
    useProjectStore.getState().updateEvent(id, data);
  }, []);

  const deleteEvent = useCallback((id: string) => {
    useProjectStore.getState().deleteEvent(id);
    closeTab(id);
  }, [closeTab]);

  // Functions
  const addFunction = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New Function", st.functions);
    const id = `func-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(
        `node-${crypto.randomUUID()}`,
        "function_entry",
        finalName,
        ["Internal"],
        { x: 50, y: 150 },
        [],
        [{ name: "Then", type: "exec" }]
      ),
      createInternalNode(
        `node-${crypto.randomUUID()}`,
        "function_return",
        "Return",
        ["Internal"],
        { x: 550, y: 150 },
        [{ name: "In", type: "exec" }],
        []
      )
    ];
    const sub: SubGraphData = {
      id,
      name: finalName,
      type: "function",
      nodes: tNodes,
      canvas: { x: 0, y: 0, scale: 1 },
      variables: {},
      inputs: [],
      outputs: []
    };
    st.addFunction(id, sub);
    openSubGraph(id, finalName, "function", sub);
    switchSidebarTab('functions');
  }, [openSubGraph, switchSidebarTab]);

  const updateFunction = useCallback((id: string, data: Partial<SubGraphData>) => {
    useProjectStore.getState().updateFunction(id, data);
  }, []);

  const deleteFunction = useCallback((id: string) => {
    useProjectStore.getState().deleteFunction(id);
    closeTab(id);
  }, [closeTab]);

  // Macros
  const addMacro = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New Macro", st.macros);
    const id = `macro-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(
        `node-${crypto.randomUUID()}`,
        "macro_inputs",
        "Inputs",
        ["Internal"],
        { x: 50, y: 150 },
        [],
        [{ name: "In", type: "exec" }]
      ),
      createInternalNode(
        `node-${crypto.randomUUID()}`,
        "macro_outputs",
        "Outputs",
        ["Internal"],
        { x: 550, y: 150 },
        [{ name: "Out", type: "exec" }],
        []
      )
    ];
    const sub: SubGraphData = {
      id,
      name: finalName,
      type: "macro",
      nodes: tNodes,
      canvas: { x: 0, y: 0, scale: 1 },
      variables: {},
      inputs: [],
      outputs: []
    };
    st.addMacro(id, sub);
    openSubGraph(id, finalName, "macro", sub);
    switchSidebarTab('macros');
  }, [openSubGraph, switchSidebarTab]);

  const updateMacro = useCallback((id: string, data: Partial<SubGraphData>) => {
    useProjectStore.getState().updateMacro(id, data);
  }, []);

  const deleteMacro = useCallback((id: string) => {
    useProjectStore.getState().deleteMacro(id);
    closeTab(id);
  }, [closeTab]);

  return {
    // Events
    addEvent,
    updateEvent,
    deleteEvent,

    // Functions
    addFunction,
    updateFunction,
    deleteFunction,

    // Macros
    addMacro,
    updateMacro,
    deleteMacro,
  };
}
