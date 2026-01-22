import { create } from 'zustand';
import { SubGraphData, ProjectData } from "../Types/canvas";
import { ProjectService } from "../../../services/projectService";
import { TabState, Variable, useNodeStore } from "./useNodeStore";
import { syncInternalNodePins, syncSubGraphInstanceNodes } from "../Utils/internalNodes";

interface ProjectStore {
    // State
    events: Record<string, SubGraphData>;
    functions: Record<string, SubGraphData>;
    macros: Record<string, SubGraphData>;
    globalVariables: Record<string, Variable>;
    currentPath: string | null;

    // Actions
    setEvents: (events: Record<string, SubGraphData>) => void;
    setFunctions: (functions: Record<string, SubGraphData>) => void;
    setMacros: (macros: Record<string, SubGraphData>) => void;
    setGlobalVariables: (vars: Record<string, Variable>) => void;
    setCurrentPath: (path: string | null) => void;

    addEvent: (id: string, data: SubGraphData) => void;
    updateEvent: (id: string, data: Partial<SubGraphData>) => void;
    deleteEvent: (id: string) => void;

    addFunction: (id: string, data: SubGraphData) => void;
    updateFunction: (id: string, data: Partial<SubGraphData>) => void;
    deleteFunction: (id: string) => void;

    addMacro: (id: string, data: SubGraphData) => void;
    updateMacro: (id: string, data: Partial<SubGraphData>) => void;
    deleteMacro: (id: string) => void;

    addGlobalVariable: (id: string, v: Variable) => void;
    updateGlobalVariable: (id: string, data: Partial<Variable>) => void;
    deleteGlobalVariable: (id: string) => void;

    // Logic
    loadProject: (project: ProjectData, path: string | null) => void;
    syncWithTabs: (tabs: Record<string, TabState>) => void;
    syncTab: (tabId: string, tabState: TabState) => void;
}

export const useProjectStore = create<ProjectStore>((set, get) => ({
    events: {},
    functions: {},
    macros: {},
    globalVariables: {},
    currentPath: null,

    setEvents: (events) => set({ events }),
    setFunctions: (functions) => set({ functions }),
    setMacros: (macros) => set({ macros }),
    setGlobalVariables: (globalVariables) => set({ globalVariables }),
    setCurrentPath: (currentPath) => set({ currentPath }),

    addEvent: (id, data) => set((state) => ({ events: { ...state.events, [id]: data } })),
    updateEvent: (id, data) => set((state) => ({ events: { ...state.events, [id]: { ...state.events[id], ...data } } })),
    deleteEvent: (id) => set((state) => {
        const next = { ...state.events };
        delete next[id];
        return { events: next };
    }),

    addFunction: (id, data) => set((state) => ({ functions: { ...state.functions, [id]: data } })),
    updateFunction: (id, data) => {
        set((state) => {
            const nextFunctions = { ...state.functions, [id]: { ...state.functions[id], ...data } };

            // Cascade update to instances in all collections
            const cascade = (collection: Record<string, SubGraphData>) => {
                const nextCollection = { ...collection };
                let changed = false;
                Object.values(nextCollection).forEach(sub => {
                    const newNodes = syncSubGraphInstanceNodes(sub.nodes, id, data.inputs, data.outputs, data.name);
                    if (newNodes !== sub.nodes) {
                        nextCollection[sub.id] = { ...sub, nodes: newNodes };
                        changed = true;
                    }
                });
                return changed ? nextCollection : collection;
            };

            const nextEvents = cascade(state.events);
            const nextMacros = cascade(state.macros);
            // recursive function calls?
            const nextFunctionsRecursive = cascade(nextFunctions);

            // Update live tabs
            const nodeStore = useNodeStore.getState();
            Object.keys(nodeStore.tabs).forEach(tid => {
                const currentNodes = nodeStore.getNodes(tid);
                const newNodes = syncSubGraphInstanceNodes(currentNodes, id, data.inputs, data.outputs, data.name);

                if (tid === id) {
                    // Update definition tab internal nodes
                    const updatedSelf = newNodes.map(n => {
                        if (!n.isInternal) return n;
                        const clone = n.clone();
                        if (data.name && (n.type === 'function_entry' || n.type === 'macro_inputs')) clone.title = data.name;
                        if (n.type === "function_entry" && data.inputs) syncInternalNodePins(clone, data.inputs, true);
                        if (n.type === "function_return" && data.outputs) syncInternalNodePins(clone, data.outputs, false);
                        return clone;
                    });
                    nodeStore.setNodes(tid, updatedSelf);
                } else if (newNodes !== currentNodes) {
                    nodeStore.setNodes(tid, newNodes);
                }
            });

            return {
                functions: nextFunctionsRecursive,
                events: nextEvents,
                macros: nextMacros
            };
        });
    },
    deleteFunction: (id) => set((state) => {
        const next = { ...state.functions };
        delete next[id];
        return { functions: next };
    }),

    addMacro: (id, data) => set((state) => ({ macros: { ...state.macros, [id]: data } })),
    updateMacro: (id, data) => {
        // Macros behave similarly to functions regarding instances
        get().updateFunction(id, data);
        // But we need to update 'macros' state specifically if updateFunction only updated 'functions' state?
        // Actually updateFunction logic above updates ALL collections via 'cascade'.
        // BUT updateFunction specifically updates 'state.functions[id]'.
        // So we need to do the same for macro definition itself.

        set((state) => {
            const nextMacros = { ...state.macros, [id]: { ...state.macros[id], ...data } };

            // Cascade update (reuse logic if possible, but for now duplicate strictly for clarity)
            const cascade = (collection: Record<string, SubGraphData>) => {
                const nextCollection = { ...collection };
                let changed = false;
                Object.values(nextCollection).forEach(sub => {
                    const newNodes = syncSubGraphInstanceNodes(sub.nodes, id, data.inputs, data.outputs, data.name);
                    if (newNodes !== sub.nodes) {
                        nextCollection[sub.id] = { ...sub, nodes: newNodes };
                        changed = true;
                    }
                });
                return changed ? nextCollection : collection;
            };

            const nextEvents = cascade(state.events);
            const nextFunctions = cascade(state.functions);
            const nextMacrosRecursive = cascade(nextMacros);

            // Update live tabs
            const nodeStore = useNodeStore.getState();
            Object.keys(nodeStore.tabs).forEach(tid => {
                const currentNodes = nodeStore.getNodes(tid);
                const newNodes = syncSubGraphInstanceNodes(currentNodes, id, data.inputs, data.outputs, data.name);

                if (tid === id) {
                    const updatedSelf = newNodes.map(n => {
                        if (!n.isInternal) return n;
                        const clone = n.clone();
                        if (data.name && (n.type === 'macro_inputs')) clone.title = data.name;
                        if (n.type === "macro_inputs" && data.inputs) syncInternalNodePins(clone, data.inputs, true);
                        if (n.type === "macro_outputs" && data.outputs) syncInternalNodePins(clone, data.outputs, false);
                        return clone;
                    });
                    nodeStore.setNodes(tid, updatedSelf);
                } else if (newNodes !== currentNodes) {
                    nodeStore.setNodes(tid, newNodes);
                }
            });

            return {
                macros: nextMacrosRecursive,
                events: nextEvents,
                functions: nextFunctions
            };
        });
    },
    deleteMacro: (id) => set((state) => {
        const next = { ...state.macros };
        delete next[id];
        return { macros: next };
    }),

    addGlobalVariable: (id, v) => set((state) => ({ globalVariables: { ...state.globalVariables, [id]: v } })),
    updateGlobalVariable: (id, data) => set((state) => ({ globalVariables: { ...state.globalVariables, [id]: { ...state.globalVariables[id], ...data } } })),
    deleteGlobalVariable: (id) => set((state) => {
        const next = { ...state.globalVariables };
        delete next[id];
        return { globalVariables: next };
    }),

    loadProject: (project, path) => set({
        events: project.events || {},
        functions: project.functions || {},
        macros: project.macros || {},
        globalVariables: project.globalVariables || {},
        currentPath: path
    }),

    syncWithTabs: (tabs) => {
        const { events, functions, macros } = get();
        // Use ProjectService to effectively clone and update based on tabs
        // Note: ProjectService.syncStoreToCollections logic requires us to pass the 'current' state
        const { nextEvents, nextFunctions, nextMacros, changed } = ProjectService.syncStoreToCollections(
            tabs,
            events,
            functions,
            macros
        );

        if (changed) {
            set({
                events: nextEvents,
                functions: nextFunctions,
                macros: nextMacros
            });
        }
    },

    syncTab: (tabId, tabState) => {
        const { events, functions, macros } = get();
        // Small efficiency update: only update the relevant collection
        const nextEvents = { ...events };
        const nextFunctions = { ...functions };
        const nextMacros = { ...macros };

        const { nextEvents: e, nextFunctions: f, nextMacros: m, changed } = ProjectService.syncStoreToCollections(
            { [tabId]: tabState },
            nextEvents,
            nextFunctions,
            nextMacros
        );

        if (changed) {
            set({
                events: e,
                functions: f,
                macros: m
            });
        }
    }
}));
