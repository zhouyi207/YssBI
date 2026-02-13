import { invoke } from "@tauri-apps/api/core";

/**
 * Graph Service - 管理 Event、Function、Macro 的创建、删除、更新和查询
 */
export class GraphService {
    /**
     * 创建 Event
     */
    static async createEvent(graph_name: string): Promise<void> {
        try {
            await invoke("create_event", { graph_name });
            console.log(`[GraphService.createEvent] Event '${graph_name}' created successfully`);
        } catch (error) {
            console.error("[GraphService.createEvent] Error creating event:", error);
            throw error;
        }
    }

    /**
     * 创建 Function
     */
    static async createFunction(graph_name: string): Promise<void> {
        try {
            await invoke("create_function", { graph_name });
            console.log(`[GraphService.createFunction] Function '${graph_name}' created successfully`);
        } catch (error) {
            console.error("[GraphService.createFunction] Error creating function:", error);
            throw error;
        }
    }

    /**
     * 创建 Macro
     */
    static async createMacro(graph_name: string): Promise<void> {
        try {
            await invoke("create_macro", { graph_name });
            console.log(`[GraphService.createMacro] Macro '${graph_name}' created successfully`);
        } catch (error) {
            console.error("[GraphService.createMacro] Error creating macro:", error);
            throw error;
        }
    }

    /**
     * 删除 Graph (Event/Function/Macro)
     */
    static async removeGraph(graphId: string): Promise<void> {
        try {
            await invoke("remove_graph", { graphId });
            console.log(`[GraphService.removeGraph] Graph '${graphId}' removed successfully`);
        } catch (error) {
            console.error("[GraphService.removeGraph] Error removing graph:", error);
            throw error;
        }
    }

    /**
     * 更新 Event
     */
    static async updateEvent(id: string, event: any): Promise<void> {
        try {
            await invoke("update_event", { id, event });
            console.log(`[GraphService.updateEvent] Event '${id}' updated successfully`);
        } catch (error) {
            console.error("[GraphService.updateEvent] Error updating event:", error);
            throw error;
        }
    }

    /**
     * 更新 Function
     */
    static async updateFunction(id: string, functionData: any): Promise<void> {
        try {
            await invoke("update_function", { id, function: functionData });
            console.log(`[GraphService.updateFunction] Function '${id}' updated successfully`);
        } catch (error) {
            console.error("[GraphService.updateFunction] Error updating function:", error);
            throw error;
        }
    }

    /**
     * 更新 Macro
     */
    static async updateMacro(id: string, macroData: any): Promise<void> {
        try {
            await invoke("update_macro", { id, macroData });
            console.log(`[GraphService.updateMacro] Macro '${id}' updated successfully`);
        } catch (error) {
            console.error("[GraphService.updateMacro] Error updating macro:", error);
            throw error;
        }
    }

    /**
     * 获取 Graph 详情
     */
    static async getGraph(graphId: string): Promise<any> {
        try {
            const graph = await invoke("get_graph", { graphId });
            console.log(`[GraphService.getGraph] Graph '${graphId}' retrieved successfully`);
            return graph;
        } catch (error) {
            console.error("[GraphService.getGraph] Error getting graph:", error);
            throw error;
        }
    }
}


// // ==================== Events CRUD ====================

//     static async getEvents(): Promise<Record<string, SubGraphData>> {
//         const data: Record<string, any> = await invoke("get_events");
//         return convertSubGraphMap(data);
//     }

//     static async getEvent(id: string): Promise<SubGraphData | null> {
//         const data: any = await invoke("get_event", { id });
//         return data ? toFrontendSubGraphData(data) : null;
//     }

//     static async createEvent(id: string, data: SubGraphData): Promise<SubGraphData> {
//         const result: any = await invoke("create_event", { id, data: toBackendSubGraphData(data) });
//         return toFrontendSubGraphData(result);
//     }

//     static async updateEvent(id: string, data: SubGraphData): Promise<SubGraphData> {
//         const result: any = await invoke("update_event", { id, data: toBackendSubGraphData(data) });
//         return toFrontendSubGraphData(result);
//     }

//     static async deleteEvent(id: string): Promise<void> {
//         await invoke("delete_event", { id });
//     }

//     // ==================== Functions CRUD ====================

//     static async getFunctions(): Promise<Record<string, SubGraphData>> {
//         const data: Record<string, any> = await invoke("get_functions");
//         return convertSubGraphMap(data);
//     }

//     static async getFunction(id: string): Promise<SubGraphData | null> {
//         const data: any = await invoke("get_function", { id });
//         return data ? toFrontendSubGraphData(data) : null;
//     }

//     static async createFunction(id: string, data: SubGraphData): Promise<SubGraphData> {
//         const result: any = await invoke("create_function", { id, data: toBackendSubGraphData(data) });
//         return toFrontendSubGraphData(result);
//     }

//     static async updateFunction(id: string, data: SubGraphData): Promise<SubGraphData> {
//         const result: any = await invoke("update_function", { id, data: toBackendSubGraphData(data) });
//         return toFrontendSubGraphData(result);
//     }

//     static async deleteFunction(id: string): Promise<void> {
//         await invoke("delete_function", { id });
//     }

//     // ==================== Macros CRUD ====================

//     static async getMacros(): Promise<Record<string, SubGraphData>> {
//         const data: Record<string, any> = await invoke("get_macros");
//         return convertSubGraphMap(data);
//     }

//     static async getMacro(id: string): Promise<SubGraphData | null> {
//         const data: any = await invoke("get_macro", { id });
//         return data ? toFrontendSubGraphData(data) : null;
//     }

//     static async createMacro(id: string, data: SubGraphData): Promise<SubGraphData> {
//         const result: any = await invoke("create_macro", { id, data: toBackendSubGraphData(data) });
//         return toFrontendSubGraphData(result);
//     }

//     static async updateMacro(id: string, data: SubGraphData): Promise<SubGraphData> {
//         const result: any = await invoke("update_macro", { id, data: toBackendSubGraphData(data) });
//         return toFrontendSubGraphData(result);
//     }

//     static async deleteMacro(id: string): Promise<void> {
//         await invoke("delete_macro", { id });
//     }