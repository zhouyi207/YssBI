import { Pin, PinDirection } from './pin';
import { DataType } from './dataType';

/**
 * Domain Types - Node
 * 
 * 这些类型代表核心业务领域模型，与后端数据结构一致
 */

export interface Node {
    id: string;
    nodeType: string;
    category: string[];
    title: string;
    inputs: Pin[];
    outputs: Pin[];
    uiStyle: string;
    description?: string;
}

export interface NodeMetaData {
    uiStyle?: string;
    /** @deprecated 后端 DTO 格式，优先使用 uiStyle */
    ui_style?: string;
    description?: string;
    supports_dynamic_pins: boolean;
}

// ─── Pin Definition Types (mirrors backend PinDefinition / PinSlot) ────

export type PinKind = 'Data' | 'Exec';

export type PinDataTypeDefinition =
    | { Concrete: DataType }
    | { TypeVar: string }
    | 'Unknown';

export type ExecRoleDTO =
    | 'ExecIn' | 'ExecOut' | 'ExecTrue' | 'ExecFalse'
    | 'ExecLoopBody' | 'ExecLoopComplete' | 'Cases'
    | { Steps: number } | { Custom: string };

export type DataRoleDTO =
    | 'Condition' | 'Input' | 'Output' | 'Result' | 'Error'
    | { Operands: number } | { Inputs: number } | { Outputs: number }
    | { Custom: string };

export type PinRoleDTO =
    | { Exec: ExecRoleDTO }
    | { Data: DataRoleDTO };

export interface PinMetaDataDTO {
    showWidget: boolean;
    widgetType: string | null;
    isDynamic: boolean;
    widgetOptions?: string[];
}

export interface PinDefinitionDTO {
    name: string;
    direction: PinDirection;
    kind: PinKind;
    role: PinRoleDTO;
    dataType: PinDataTypeDefinition | null;
    metaData: PinMetaDataDTO;
}

export type PinSlot =
    | { slotKind: 'fixed'; pin: PinDefinitionDTO }
    | { slotKind: 'repeatable'; template: PinDefinitionDTO; namePrefix: string; minCount: number; maxCount: number | null }
    | { slotKind: 'derivedFromInput'; sourceRole: PinRoleDTO; direction: PinDirection; baseType: PinDataTypeDefinition };

export interface PinTypeCapability {
    direction: PinDirection;
    kind: PinKind;
    dataType: PinDataTypeDefinition;
}

// ─── Node Definition DTO ────

export interface NodeDefinitionDTO {
    name: string;
    category: string[];
    nodeType: string;
    node_metadata: NodeMetaData;
    pinSlots: PinSlot[];
    typeCapabilities: PinTypeCapability[];
}

export type NodeDefinition = NodeDefinitionDTO;

export interface NodePosition {
    x: number;
    y: number;
}
