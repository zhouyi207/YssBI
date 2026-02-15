/**
 * DTO Converters
 * 
 * 前后端数据转换器
 * 用于在前端类型和后端 DTO 之间进行转换
 * 
 * 当前实现：
 * - 大部分类型前后端一致，转换器直接返回原对象
 * - 为未来可能的差异预留扩展点
 */

import type {
    NodeDefinition,
    NodeDefinitionDTO,
    Pin,
    Connection,
    Graph,
    Variable,
    ProjectData,
    DataSourceConfig,
} from '../domain';

/**
 * 转换器接口
 */
export interface Converter<T, D = T> {
    fromDTO(dto: D): T;
    toDTO(data: T): D;
}

/**
 * 节点转换器
 */
export const NodeConverter: Converter<NodeDefinition, NodeDefinitionDTO> = {
    fromDTO(dto: NodeDefinitionDTO): NodeDefinition {
        return dto;
    },

    toDTO(node: NodeDefinition): NodeDefinitionDTO {
        return node;
    },
};

/**
 * Pin 转换器
 */
export const PinConverter: Converter<Pin> = {
    fromDTO(dto: Pin): Pin {
        return dto;
    },

    toDTO(pin: Pin): Pin {
        return pin;
    },
};

/**
 * 连接转换器
 */
export const ConnectionConverter: Converter<Connection> = {
    fromDTO(dto: Connection): Connection {
        return dto;
    },

    toDTO(connection: Connection): Connection {
        return connection;
    },
};

/**
 * 图转换器
 */
export const GraphConverter: Converter<Graph> = {
    fromDTO(dto: Graph): Graph {
        return dto;
    },

    toDTO(graph: Graph): Graph {
        return graph;
    },
};

/**
 * 变量转换器
 */
export const VariableConverter: Converter<Variable> = {
    fromDTO(dto: Variable): Variable {
        return dto;
    },

    toDTO(variable: Variable): Variable {
        return variable;
    },
};

/**
 * 项目数据转换器
 */
export const ProjectDataConverter: Converter<ProjectData> = {
    fromDTO(dto: ProjectData): ProjectData {
        return dto;
    },

    toDTO(data: ProjectData): ProjectData {
        return data;
    },
};

/**
 * 数据库转换器
 */
export const DatabaseConverter: Converter<DataSourceConfig> = {
    fromDTO(dto: DataSourceConfig): DataSourceConfig {
        return dto;
    },

    toDTO(config: DataSourceConfig): DataSourceConfig {
        return config;
    },
};
