import { NodeDefinition, NodeDefinitionDTO } from "@/shared/types";
import { invoke } from "@tauri-apps/api/core";
import { logger } from '@/utils/appLogger';

export interface EditorSchemaDTO {
    nodeDefinitions: NodeDefinition[];
}

export class SchemaService {
    static async getNodeDefinition(): Promise<NodeDefinition[]> {
        logger.sys.debug('Calling backend...', 'SchemaService.getNodeDefinition');
        try {
            const node_definition_list = await invoke<NodeDefinitionDTO[]>("get_node_definitions");
            logger.sys.info('Success! Received nodes len: ' + node_definition_list.length, 'SchemaService.getNodeDefinition');
            return node_definition_list;
        } catch (error) {
            logger.sys.error('Failed to get node definitions: ' + (error instanceof Error ? error.message : String(error)), 'SchemaService.getNodeDefinition');
            throw error;
        }
    }

    static async getEditorSchema(): Promise<EditorSchemaDTO> {
        return invoke<EditorSchemaDTO>("get_editor_schema_command");
    }
}