import { Pin, PinDirection } from './pin';
import { DataType } from './dataType';

export type { PinDirection };

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
}

export type NodeGraphScope = 'any' | 'event' | 'function';
export type ShellRole = 'event_begin' | 'function_entry' | 'function_return';
export type NodeDefinitionOrigin = 'system' | 'custom';

export interface NodeMetaData {
    uiStyle: string;
    documentation?: NodeDocumentation;
    supports_dynamic_pins: boolean;
    /** 节点允许出现的图类型。 */
    graph_scope: NodeGraphScope;
    /** 系统托管壳节点角色；非 null 即为壳节点（不可删 / 复制 / palette 隐藏）。 */
    shell_role: ShellRole | null;
    /** 定义来源：系统注册表 vs 用户/项目自定义。缺省为 system。 */
    definition_origin?: NodeDefinitionOrigin;
}

export interface NodeDocumentation {
    zh?: string;
    en?: string;
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
    optional?: boolean;
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
    nodeMetadata: NodeMetaData;
    pinSlots: PinSlot[];
    typeCapabilities: PinTypeCapability[];
}

export function getNodeDefinitionMeta(def: NodeDefinition | undefined): NodeMetaData | undefined {
    return def?.nodeMetadata;
}

/** 系统托管壳节点（Event Begin / Function Entry/Return）：不可删除、不从 palette 添加。 */
export function isShellNodeDefinition(def: NodeDefinition | undefined): boolean {
    return getNodeDefinitionMeta(def)?.shell_role != null;
}

/** 该节点定义允许出现在指定图类型中（默认 any）。 */
export function nodeDefinitionAllowedInGraphKind(
    def: NodeDefinition | undefined,
    graphKind: 'event' | 'function' | undefined,
): boolean {
    const scope = getNodeDefinitionMeta(def)?.graph_scope ?? 'any';
    if (scope === 'any' || !graphKind) return true;
    return scope === graphKind;
}

/** 用户/项目自定义节点定义（`definition_origin === 'custom'`）。 */
export function isCustomNodeDefinition(def: NodeDefinition | undefined): boolean {
    return getNodeDefinitionMeta(def)?.definition_origin === 'custom';
}

export function pickLocalizedText(
    text: NodeDocumentation | undefined,
    language: string,
): string | undefined {
    if (!text) return undefined;
    const isZh = language.startsWith('zh');
    const primary = isZh ? text.zh : text.en;
    const fallback = isZh ? text.en : text.zh;
    return primary ?? fallback;
}

export type NodeDefinition = NodeDefinitionDTO;

export interface NodePosition {
    x: number;
    y: number;
}
