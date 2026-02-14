import { NodeDefinition, NodeDefinitionDTO } from "@/shared/types";
import { invoke } from "@tauri-apps/api/core";

export class SchemaService {
    static async getNodeDefinition(): Promise<NodeDefinition[]> {
        console.log("[SchemaService.getNodeDefinition] Calling backend...");
        try {
            const node_definition_list = await invoke<NodeDefinitionDTO[]>("get_node_definitions");
            console.info("[SchemaService.getNodeDefinition] Success! Received nodes len: ", node_definition_list.length);
            return node_definition_list;
        } catch (error) {
            console.error("[SchemaService.getNodeDefinition] Failed to get node definitions:", error);
            throw error;
        }
    }
}