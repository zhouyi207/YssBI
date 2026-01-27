import { create } from 'zustand';
import { SubGraphData, ProjectData } from "../Types/canvas";
import { VariableDefinition } from "../Types/variables";
import { ProjectService } from "../../../services/projectService";
import { TabState, useNodeStore } from "./useNodeStore";
import { syncInternalNodePins, syncSubGraphInstanceNodes } from "../Utils/internalNodes";

interface ProjectStore {
    // State (作为后端数据的缓存)
    events: Record<string, SubGraphData>;
    functions: Record<string, SubGraphData>;
    macros: Record<string, SubGraphData>;
    globalVariables: Record<string, VariableDefinition>;
    currentPath: string | null;

    // 内部 Setters (供事件订阅使用)
    setEvents: (events: Record<string, SubGraphData>) => void;
    setFunctions: (functions: Record<string, SubGraphData>) => void;
    setMacros: (macros: Record<string, SubGraphData>) => void;
    setGlobalVariables: (vars: Record<string, VariableDefinition>) => void;
    setCurrentPath: (path: string | null) => void;

    // Event 操作 (调用后端 API)
    addEvent: (id: string, data: SubGraphData) => void;
    updateEvent: (id: string, data: Partial<SubGraphData>) => void;
    deleteEvent: (id: string) => void;

    // Function 操作 (调用后端 API)
    addFunction: (id: string, data: SubGraphData) => void;
    updateFunction: (id: string, data: Partial<SubGraphData>) => void;
    deleteFunction: (id: string) => void;

    // Macro 操作 (调用后端 API)
    addMacro: (id: string, data: SubGraphData) => void;
    updateMacro: (id: string, data: Partial<SubGraphData>) => void;
    deleteMacro: (id: string) => void;

    // Global Variable 操作 (调用后端 API)
    addGlobalVariable: (id: string, v: VariableDefinition) => void;
    updateGlobalVariable: (id: string, data: Partial<VariableDefinition>) => void;
    deleteGlobalVariable: (id: string) => void;

    // 项目级操作
    loadProject: (project: ProjectData, path: string | null) => void;
    syncWithTabs: (tabs: Record<string, TabState>) => void;
    syncTab: (tabId: string, tabState: TabState) => void;

    // 同步到后端
    syncToBackend: () => Promise<void>;
}

export const useProjectStore = create<ProjectStore>((set, get) => ({
    events: {},
    functions: {},
    macros: {},
    globalVariables: {},
    currentPath: null,

    // 内部 Setters
    setEvents: (events) => set({ events }),
    setFunctions: (functions) => set({ functions }),
    setMacros: (macros) => set({ macros }),
    setGlobalVariables: (globalVariables) => set({ globalVariables }),
    setCurrentPath: (currentPath) => set({ currentPath }),

    // Event 操作
    addEvent: (id, data) => {
        // 先更新本地状态以获得即时反馈
        set((state) => ({ events: { ...state.events, [id]: data } }));
        // 异步同步到后端
        ProjectService.createEvent(id, data).catch(console.error);
    },

    updateEvent: (id, data) => {
        set((state) => ({ events: { ...state.events, [id]: { ...state.events[id], ...data } } }));
        // 获取完整数据并同步到后端
        const fullData = get().events[id];
        if (fullData) {
            ProjectService.updateEvent(id, fullData).catch(console.error);
        }
    },

    deleteEvent: (id) => {
        set((state) => {
            const next = { ...state.events };
            delete next[id];
            return { events: next };
        });
        ProjectService.deleteEvent(id).catch(console.error);
    },

    // Function 操作
    addFunction: (id, data) => {
        set((state) => ({ functions: { ...state.functions, [id]: data } }));
        ProjectService.createFunction(id, data).catch(console.error);
    },

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
            const nextFunctionsRecursive = cascade(nextFunctions);

            // Update live tabs
            const nodeStore = useNodeStore.getState();
            Object.keys(nodeStore.tabs).forEach(tid => {
                const currentNodes = nodeStore.getNodes(tid);
                const newNodes = syncSubGraphInstanceNodes(currentNodes, id, data.inputs, data.outputs, data.name);

                if (tid === id) {
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

        // 同步到后端
        const fullData = get().functions[id];
        if (fullData) {
            ProjectService.updateFunction(id, fullData).catch(console.error);
        }
    },

    deleteFunction: (id) => {
        set((state) => {
            const next = { ...state.functions };
            delete next[id];
            return { functions: next };
        });
        ProjectService.deleteFunction(id).catch(console.error);
    },

    // Macro 操作
    addMacro: (id, data) => {
        set((state) => ({ macros: { ...state.macros, [id]: data } }));
        ProjectService.createMacro(id, data).catch(console.error);
    },

    updateMacro: (id, data) => {
        set((state) => {
            const nextMacros = { ...state.macros, [id]: { ...state.macros[id], ...data } };

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

        // 同步到后端
        const fullData = get().macros[id];
        if (fullData) {
            ProjectService.updateMacro(id, fullData).catch(console.error);
        }
    },

    deleteMacro: (id) => {
        set((state) => {
            const next = { ...state.macros };
            delete next[id];
            return { macros: next };
        });
        ProjectService.deleteMacro(id).catch(console.error);
    },

    // Global Variable 操作
    addGlobalVariable: (id, v) => {
        set((state) => ({ globalVariables: { ...state.globalVariables, [id]: v } }));
        ProjectService.createGlobalVariable(id, v).catch(console.error);
    },

    updateGlobalVariable: (id, data) => {
        set((state) => ({ globalVariables: { ...state.globalVariables, [id]: { ...state.globalVariables[id], ...data } } }));
        const fullData = get().globalVariables[id];
        if (fullData) {
            ProjectService.updateGlobalVariable(id, fullData).catch(console.error);
        }
    },

    deleteGlobalVariable: (id) => {
        set((state) => {
            const next = { ...state.globalVariables };
            delete next[id];
            return { globalVariables: next };
        });
        ProjectService.deleteGlobalVariable(id).catch(console.error);
    },

    // 项目级操作
    loadProject: (project, path) => set({
        events: project.events || {},
        functions: project.functions || {},
        macros: project.macros || {},
        globalVariables: project.globalVariables || {},
        currentPath: path
    }),

    syncWithTabs: (tabs) => {
        const { events, functions, macros } = get();
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

            // 同步到后端
            get().syncToBackend().catch(console.error);
        }
    },

    syncTab: (tabId, tabState) => {
        const { events, functions, macros } = get();
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

            // 同步单个子图到后端
            const updatedSubGraph = e[tabId] || f[tabId] || m[tabId];
            if (updatedSubGraph) {
                if (e[tabId]) {
                    ProjectService.updateEvent(tabId, updatedSubGraph).catch(console.error);
                } else if (f[tabId]) {
                    ProjectService.updateFunction(tabId, updatedSubGraph).catch(console.error);
                } else if (m[tabId]) {
                    ProjectService.updateMacro(tabId, updatedSubGraph).catch(console.error);
                }
            }
        }
    },

    // 批量同步到后端
    syncToBackend: async () => {
        const { events, functions, macros, globalVariables, currentPath } = get();
        const projectData: ProjectData = {
            events,
            functions,
            macros,
            globalVariables,
            metadata: {
                exportTime: new Date().toISOString(),
                appVersion: "0.1.0"
            }
        };
        console.log('[SyncToBackend] Syncing project data to backend:', {
            eventsCount: Object.keys(events).length,
            functionsCount: Object.keys(functions).length,
            macrosCount: Object.keys(macros).length,
            globalVariablesCount: Object.keys(globalVariables).length,
        });
        try {
            // 自动同步时不触发事件（避免循环）
            await ProjectService.setProjectData(projectData, currentPath || undefined, false);
            console.log('[SyncToBackend] Successfully synced to backend');
        } catch (e) {
            console.error('[SyncToBackend] Failed to sync to backend:', e);
            throw e;
        }
    }
}));
