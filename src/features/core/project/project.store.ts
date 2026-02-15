/// store —— 只负责「状态 + backend 同步」

import { create } from 'zustand';
import { LoadStatus } from '@/shared/types/ui';
import { ProjectState } from '@/shared/types/domain';
import { Graph, ProjectData } from '@/shared/types/domain';
import { Variable } from '@/shared/types/domain';
import { ProjectService } from '@/services/project/projectService';

interface ProjectStore extends ProjectState {
    // Project Data (完全匹配 ProjectData 类型定义)
    variables: Record<string, Variable>;
    graphs: Record<string, Graph>;
    databases: Record<string, any>;
    currentPath: string | null;

    // Backend Sync
    syncFromBackend: () => Promise<ProjectData | null>;
    syncToBackend: () => Promise<void>;
    clear: () => void;

    // Setters
    setVariables: (vars: Record<string, Variable>) => void;
    setGraphs: (graphs: Record<string, Graph>) => void;
    setDatabases: (dbs: Record<string, any>) => void;
    setCurrentPath: (path: string | null) => void;


    // Variable 操作
    addVariable: (id: string, variable: Variable) => void;
    updateVariable: (id: string, data: Partial<Variable>) => void;
    deleteVariable: (id: string) => void;

    // Graph 操作 (Event/Function/Macro)
    addGraph: (id: string, graph: Graph) => void;
    updateGraph: (id: string, data: Partial<Graph>) => void;
    deleteGraph: (id: string) => void;

    // Database 操作
    addDatabase: (id: string, db: any) => void;
    updateDatabase: (id: string, data: Partial<any>) => void;
    deleteDatabase: (id: string) => void;

    // 项目级操作
    loadProject: (project: ProjectData, path: string | null) => void;
}

export const useProjectStore = create<ProjectStore>((set, get) => ({
    // 核心数据 (完全匹配 ProjectData 类型)
    variables: {},
    graphs: {},
    databases: {},
    currentPath: null,

    // state
    status: LoadStatus.Idle,
    error: null,

    syncFromBackend: async () => {
        const { status } = get();

        // 只在正在加载时跳过，允许在 Ready 状态下重新同步
        if (status === LoadStatus.Loading) {
            console.log('[Project] Already loading, skipping...');
            return null;
        }

        const startTime = performance.now();
        console.log('[Project] Syncing project state from backend...');

        set({ status: LoadStatus.Loading, error: null });

        try {
            const projectData = await ProjectService.getProjectState();
            const path = await ProjectService.getProjectPath();

            // 调试：打印后端返回的原始数据
            console.log('[Project] Backend returned data:', {
                variablesCount: Object.keys(projectData.variables || {}).length,
                graphsCount: Object.keys(projectData.graphs || {}).length,
                databasesCount: Object.keys(projectData.databases || {}).length,
                graphsKeys: Object.keys(projectData.graphs || {}),
                firstGraph: Object.values(projectData.graphs || {})[0]
            });

            // 强制创建新对象引用以触发 React 更新
            const newVariables = { ...(projectData.variables || {}) };
            const newGraphs = { ...(projectData.graphs || {}) };
            const newDatabases = { ...(projectData.databases || {}) };

            set({
                variables: newVariables,
                graphs: newGraphs,
                databases: newDatabases,
                currentPath: path,
                status: LoadStatus.Ready,
            });

            const duration = performance.now() - startTime;
            
            // 计算分组数量用于日志
            let eventsCount = 0, functionsCount = 0, macrosCount = 0;
            for (const graph of Object.values(newGraphs)) {
                if (graph.type === 'event') eventsCount++;
                else if (graph.type === 'function') functionsCount++;
                else if (graph.type === 'macro') macrosCount++;
            }

            console.log('[Project] ✓ Project state synced successfully', {
                events: eventsCount,
                functions: functionsCount,
                macros: macrosCount,
                Variables: Object.keys(newVariables).length,
                dataframes: Object.keys(newDatabases).length,
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
        const { variables, graphs, databases, currentPath } = get();
        const projectData: ProjectData = {
            variables,
            graphs,
            databases,
            metadata: {
                exportTime: new Date().toISOString(),
                appVersion: "0.1.0"
            }
        };

        console.log('[Project] Syncing to backend:', {
            variables: Object.keys(variables).length,
            graphs: Object.keys(graphs).length,
            databases: Object.keys(databases).length,
        });

        try {
            await ProjectService.setProjectData(projectData, currentPath || undefined, false);
            console.log('[Project] ✓ Successfully synced to backend');
        } catch (e) {
            console.error('[Project] ✗ Failed to sync to backend:', e);
            throw e;
        }
    },

    clear: () =>
        set({
            variables: {},
            graphs: {},
            databases: {},
            currentPath: null,
            status: LoadStatus.Idle,
            error: null,
        }),

    // Setters
    setVariables: (variables) => set({ variables }),
    setGraphs: (graphs) => set({ graphs }),
    setDatabases: (databases) => set({ databases }),
    setCurrentPath: (currentPath) => set({ currentPath }),



    // Variable 操作
    addVariable: (id, variable) => {
        set((state) => ({
            variables: { ...state.variables, [id]: variable }
        }));
    },

    updateVariable: (id, data) => {
        set((state) => ({
            variables: {
                ...state.variables,
                [id]: { ...state.variables[id], ...data }
            }
        }));
    },

    deleteVariable: (id) => {
        set((state) => {
            const newVariables = { ...state.variables };
            delete newVariables[id];
            return { variables: newVariables };
        });
    },

    // Graph 操作 (Event/Function/Macro)
    addGraph: (id, graph) => {
        console.log('[ProjectStore] addGraph:', id, graph);
        set((state) => {
            const newGraphs = { ...state.graphs, [id]: graph };
            console.log('[ProjectStore] New graphs:', newGraphs);
            return { graphs: newGraphs };
        });
    },

    updateGraph: (id, data) => {
        set((state) => ({
            graphs: {
                ...state.graphs,
                [id]: { ...state.graphs[id], ...data }
            }
        }));
    },

    deleteGraph: (id) => {
        set((state) => {
            const newGraphs = { ...state.graphs };
            delete newGraphs[id];
            return { graphs: newGraphs };
        });
    },

    // Database 操作
    addDatabase: (id, db) => {
        set((state) => ({
            databases: { ...state.databases, [id]: db }
        }));
    },

    updateDatabase: (id, data) => {
        set((state) => ({
            databases: {
                ...state.databases,
                [id]: { ...state.databases[id], ...data }
            }
        }));
    },

    deleteDatabase: (id) => {
        set((state) => {
            const newDatabases = { ...state.databases };
            delete newDatabases[id];
            return { databases: newDatabases };
        });
    },

    // 项目级操作
    loadProject: (project, path) => {
        set({
            variables: project.variables || {},
            graphs: project.graphs || {},
            databases: project.databases || {},
            currentPath: path,
        });
    },
}));

// 选择器函数 - 用于从 graphs 中按类型筛选
// 使用缓存来避免每次都创建新对象导致无限循环
const graphsCache = new WeakMap<Record<string, Graph>, {
    events: Record<string, Graph>;
    functions: Record<string, Graph>;
    macros: Record<string, Graph>;
}>();

export const selectEvents = (state: ProjectStore): Record<string, Graph> => {
    const graphs = state.graphs;
    
    // 检查缓存
    let cached = graphsCache.get(graphs);
    if (!cached) {
        // 创建新的分类对象
        const events: Record<string, Graph> = {};
        const functions: Record<string, Graph> = {};
        const macros: Record<string, Graph> = {};
        
        for (const [id, graph] of Object.entries(graphs)) {
            if (graph.type === 'event') {
                events[id] = graph;
            } else if (graph.type === 'function') {
                functions[id] = graph;
            } else if (graph.type === 'macro') {
                macros[id] = graph;
            }
        }
        
        cached = { events, functions, macros };
        graphsCache.set(graphs, cached);
    }
    
    return cached.events;
};

export const selectFunctions = (state: ProjectStore): Record<string, Graph> => {
    const graphs = state.graphs;
    let cached = graphsCache.get(graphs);
    if (!cached) {
        // 如果没有缓存，调用 selectEvents 会创建缓存
        selectEvents(state);
        cached = graphsCache.get(graphs)!;
    }
    return cached.functions;
};

export const selectMacros = (state: ProjectStore): Record<string, Graph> => {
    const graphs = state.graphs;
    let cached = graphsCache.get(graphs);
    if (!cached) {
        // 如果没有缓存，调用 selectEvents 会创建缓存
        selectEvents(state);
        cached = graphsCache.get(graphs)!;
    }
    return cached.macros;
};

// 别名选择器 - 直接返回引用，不创建新对象
export const selectGlobalVariables = (state: ProjectStore): Record<string, Variable> => state.variables;
export const selectDataframes = (state: ProjectStore): Record<string, any> => state.databases;
