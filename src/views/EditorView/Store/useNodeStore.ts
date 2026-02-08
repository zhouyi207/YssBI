import { useCallback } from 'react';
import { create } from 'zustand';
import { BaseNode } from '../Types/nodes';
import { VariableDefinition } from '../Types/variables';

interface TabSnapshot {
  nodes: BaseNode[];
  variables: Record<string, VariableDefinition>;
}

export interface TabState {
  nodes: BaseNode[];
  variables: Record<string, VariableDefinition>;
  history: {
    past: TabSnapshot[];
    future: TabSnapshot[];
  };
}

interface NodeStore {
  tabs: Record<string, TabState>;

  // Lifecycle
  initTab: (tabId: string, nodes: BaseNode[], variables: Record<string, VariableDefinition>) => void;
  clearTabs: () => void;

  // Nodes
  setNodes: (tabId: string, nodes: BaseNode[]) => void;
  updateNode: (tabId: string, nodeId: string, updater: (prev: BaseNode) => BaseNode) => void;
  updateNodePosition: (tabId: string, nodeId: string, dx: number, dy: number) => void;

  // Variables
  setVariables: (tabId: string, variables: Record<string, VariableDefinition>) => void;
  updateVariable: (tabId: string, varId: string, data: Partial<VariableDefinition>) => void;
  addVariable: (tabId: string, varId: string, variable: VariableDefinition) => void;
  removeVariable: (tabId: string, varId: string) => void;

  // History
  saveSnapshot: (tabId: string) => void;
  undo: (tabId: string) => void;
  redo: (tabId: string) => void;

  // Getters
  getNodes: (tabId: string) => BaseNode[];
}

const createTabState = (nodes: BaseNode[] = [], variables: Record<string, VariableDefinition> = {}): TabState => ({
  nodes,
  variables,
  history: { past: [], future: [] }
});

export const useNodeStore = create<NodeStore>((set, get) => ({
  tabs: {},

  initTab: (tabId, nodes, variables) => set(state => ({
    tabs: { ...state.tabs, [tabId]: createTabState(nodes, variables) }
  })),

  clearTabs: () => set({ tabs: {} }),

  setNodes: (tabId, nodesArray) => set(state => {
    const tab = state.tabs[tabId];
    if (!tab) return { tabs: { ...state.tabs, [tabId]: createTabState(nodesArray) } };
    return {
      tabs: { ...state.tabs, [tabId]: { ...tab, nodes: nodesArray } }
    };
  }),

  updateNode: (tabId, nodeId, updater) => set(state => {
    const tab = state.tabs[tabId];
    if (!tab) return state;
    const idx = tab.nodes.findIndex(n => n.id === nodeId);
    if (idx === -1) return state;

    const newNodes = [...tab.nodes];
    newNodes[idx] = updater(newNodes[idx]);
    return { tabs: { ...state.tabs, [tabId]: { ...tab, nodes: newNodes } } };
  }),

  updateNodePosition: (tabId, nodeId, dx, dy) => set(state => {
    const tab = state.tabs[tabId];
    if (!tab) return state;
    const idx = tab.nodes.findIndex(n => n.id === nodeId);
    if (idx === -1) return state;

    const n = tab.nodes[idx];
    const newNode = n.clone();
    newNode.position = { x: n.position.x + dx, y: n.position.y + dy };
    const newNodes = [...tab.nodes];
    newNodes[idx] = newNode;

    return { tabs: { ...state.tabs, [tabId]: { ...tab, nodes: newNodes } } };
  }),

  setVariables: (tabId, variables) => set(state => {
    const tab = state.tabs[tabId];
    if (!tab) return state;
    return { tabs: { ...state.tabs, [tabId]: { ...tab, variables } } };
  }),

  addVariable: (tabId, varId, variable) => set(state => {
    const tab = state.tabs[tabId];
    if (!tab) return state;
    return { tabs: { ...state.tabs, [tabId]: { ...tab, variables: { ...tab.variables, [varId]: variable } } } };
  }),

  updateVariable: (tabId, varId, data) => set(state => {
    const tab = state.tabs[tabId];
    if (!tab) return state;
    const v = tab.variables[varId];
    if (!v) return state;
    return { tabs: { ...state.tabs, [tabId]: { ...tab, variables: { ...tab.variables, [varId]: { ...v, ...data } } } } };
  }),

  removeVariable: (tabId, varId) => set(state => {
    const tab = state.tabs[tabId];
    if (!tab) return state;
    const newVars = { ...tab.variables };
    delete newVars[varId];
    return { tabs: { ...state.tabs, [tabId]: { ...tab, variables: newVars } } };
  }),

  saveSnapshot: (tabId) => set(state => {
    const tab = state.tabs[tabId];
    if (!tab) return state;
    // Deep clone nodes and variables
    const snapshot: TabSnapshot = {
      nodes: tab.nodes.map(n => n.clone()),
      variables: JSON.parse(JSON.stringify(tab.variables))
    };
    const past = [...tab.history.past, snapshot].slice(-50);
    return { tabs: { ...state.tabs, [tabId]: { ...tab, history: { past, future: [] } } } };
  }),

  undo: (tabId) => set(state => {
    const tab = state.tabs[tabId];
    if (!tab || tab.history.past.length === 0) return state;
    const past = [...tab.history.past];
    const prev = past.pop()!;
    const current: TabSnapshot = {
      nodes: tab.nodes.map(n => n.clone()),
      variables: JSON.parse(JSON.stringify(tab.variables))
    };
    return {
      tabs: {
        ...state.tabs,
        [tabId]: {
          ...tab,
          nodes: prev.nodes,
          variables: prev.variables,
          history: { past, future: [current, ...tab.history.future] }
        }
      }
    };
  }),

  redo: (tabId) => set(state => {
    const tab = state.tabs[tabId];
    if (!tab || tab.history.future.length === 0) return state;
    const future = [...tab.history.future];
    const next = future.shift()!;
    const current: TabSnapshot = {
      nodes: tab.nodes.map(n => n.clone()),
      variables: JSON.parse(JSON.stringify(tab.variables))
    };
    return {
      tabs: {
        ...state.tabs,
        [tabId]: {
          ...tab,
          nodes: next.nodes,
          variables: next.variables,
          history: { past: [...tab.history.past, current], future }
        }
      }
    };
  }),

  getNodes: (tabId) => get().tabs[tabId]?.nodes || [],
}));

const EMPTY_NODES: BaseNode[] = [];
const EMPTY_VARS: Record<string, VariableDefinition> = {};

export const useTabNodes = (tabId: string | null) => {
  const selector = useCallback((state: NodeStore) => {
    if (!tabId) return EMPTY_NODES;
    return state.tabs[tabId]?.nodes || EMPTY_NODES;
  }, [tabId]);
  return useNodeStore(selector);
};

export const useTabVariables = (tabId: string | null) => {
  const selector = useCallback((state: NodeStore) => {
    if (!tabId) return EMPTY_VARS;
    return state.tabs[tabId]?.variables || EMPTY_VARS;
  }, [tabId]);
  return useNodeStore(selector);
};
