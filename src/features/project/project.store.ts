/// store —— 只负责「状态 + backend 同步」

import { create } from 'zustand';
import { LoadStatus } from '@/shared/types/loadStatus';
import { ProjectState } from './project.types';
import { SubGraphData, ProjectData, DataFrameData } from '@/shared/types/editor';
import { VariableDefinition } from '@/shared/types/editor';
import { ProjectService } from '@/services/project/projectService';
import { TabState, useNodeStore } from '@/features/node-registry/stores/useNodeStore';
import { syncInternalNodePins, syncSubGraphInstanceNodes } from '@/shared/utils/editor';

interface ProjectStore extends ProjectState {
    // Project Data (作为后端数据的缓存)
    events: Record<string, SubGraphData>;
    functions: Record<string, SubGraphData>;
    macros: Record<string, SubGraphData>;
    globalVariables: Record<string, VariableDefinition>;
    dataframes: Record<string, DataFrameData>;
    currentPath: string | null;

    // Backend Sync
    syncFromBackend: () => Promise<ProjectData | null>;
    syncToBackend: () => Promise<void>;
    clear: () => void;

    // 内部 Setters (供事件订阅使用)
    setEvents: (events: Record<string, SubGraphData>) => void;
    setFunctions: (functions: Record<string, SubGraphData>) => void;
    setMacros: (macros: Record<string, SubGraphData>) => void;
    setGlobalVariables: (vars: Record<string, VariableDefinition>) => void;
    setDataFrames: (dfs: Record<string, DataFrameData>) => void;
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

    // DataFrame 操作
    addDataFrame: (id: string, df: DataFrameData) => void;
    updateDataFrame: (id: string, data: Partial<DataFrameData>) => void;
    deleteDataFrame: (id: string) => void;

    // 项目级操作
    loadProject: (project: ProjectData, path: string | null) => void;
    syncWithTabs: (tabs: Record<string, TabState>) => void;
    syncTab: (tabId: string, tabState: TabState) => void;
}

export const useProjectStore = create<ProjectStore>((set, get) => ({
    // data
    events: {},
    functions: {},
    macros: {},
    globalVariables: {},
    dataframes: {},
    currentPath: null,

    // state (来自 ProjectState)
    status: LoadStatus.Idle,
    error: null,

    syncFromBackend: async () => {
        const { status } = get();

        // 幂等保护
        if (status === LoadStatus.Loading || status === LoadStatus.Ready) {
            console.log('[Project] Already loading or loaded, skipping...');
            return null;
        }

        const startTime = performance.now();
        console.log('[Project] Syncing project state from backend...');

        set({ status: LoadStatus.Loading, error: null });

        try {
            const projectData = await ProjectService.getProjectState();
            const path = await ProjectService.getProjectPath();

            set({
                events: projectData.events || {},
                functions: projectData.functions || {},
                macros: projectData.macros || {},
                globalVariables: projectData.globalVariables || {},
                dataframes: projectData.dataframes || {},
                currentPath: path,
                status: LoadStatus.Ready,
            });

            const duration = performance.now() - startTime;
            console.log('[Project] ✓ Project state synced successfully', {
                events: Object.keys(projectData.events || {}).length,
                functions: Object.keys(projectData.functions || {}).length,
                macros: Object.keys(projectData.macros || {}).length,
                globalVariables: Object.keys(projectData.globalVariables || {}).length,
                dataframes: Object.keys(projectData.dataframes || {}).length,
                duration: `${duration.toFixed(0)}ms`,
            });

            return projectData;
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            console.error('[Project] ✗ Failed to sync project state:', errorMessage);

            set({
                status: LoadStatus.Error,
                error: errorMessage,
            });

            return null;
        }
    },

    syncToBackend: async () => {
        const { events, functions, macros, globalVariables, dataframes, currentPath } = get();
        const projectData: ProjectData = {
            events,
            functions,
            macros,
            globalVariables,
            dataframes,
            metadata: {
                exportTime: new Date().toISOString(),
                appVersion: "0.1.0"
            }
        };

        console.log('[Project] Syncing to backend:', {
            events: Object.keys(events).length,
            functions: Object.keys(functions).length,
            macros: Object.keys(macros).length,
            globalVariables: Object.keys(globalVariables).length,
            dataframes: Object.keys(dataframes || {}).length,
        });

        try {
            // 自动同步时不触发事件（避免循环）
            await ProjectService.setProjectData(projectData, currentPath || undefined, false);
            console.log('[Project] ✓ Successfully synced to backend');
        } catch (e) {
            console.error('[Project] ✗ Failed to sync to backend:', e);
            throw e;
        }
    },

    clear: () =>
        set({
            events: {},
            functions: {},
            macros: {},
            globalVariables: {},
            dataframes: {},
            currentPath: null,
            status: LoadStatus.Idle,
            error: null,
        }),

    // 内部 Setters
    setEvents: (events) => set({ events }),
    setFunctions: (functions) => set({ functions }),
    setMacros: (macros) => set({ macros }),
    setGlobalVariables: (globalVariables) => set({ globalVariables }),
    setDataFrames: (dataframes) => set({ dataframes }),
    setCurrentPath: (currentPath) => set({ currentPath }),

    // Event 操作
    addEvent: (id, data) => {
        set((state) => ({ events: { ...state.events, [id]: data } }));
        ProjectService.createEvent(id, data).catch(console.error);
    },

    updateEvent: (id, data) => {
        set((state) => ({ events: { ...state.events, [id]: { ...state.events[id], ...data } } }));
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

    // DataFrame 操作
    addDataFrame: (id, df) => {
        set((state) => ({ dataframes: { ...state.dataframes, [id]: df } }));
        ProjectService.createDataFrame(id, df).catch(console.error);
    },

    updateDataFrame: (id, data) => {
        set((state) => ({
            dataframes: {
                ...state.dataframes,
                [id]: { ...state.dataframes[id], ...data }
            }
        }));
        get().syncToBackend().catch(console.error);
    },

    deleteDataFrame: (id) => {
        set((state) => {
            const next = { ...state.dataframes };
            delete next[id];
            return { dataframes: next };
        });
        ProjectService.deleteDataFrame(id).catch(console.error);
    },

    // 项目级操作
    loadProject: (project, path) => set({
        events: project.events || {},
        functions: project.functions || {},
        macros: project.macros || {},
        globalVariables: project.globalVariables || {},
        dataframes: project.dataframes || {},
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
}));
