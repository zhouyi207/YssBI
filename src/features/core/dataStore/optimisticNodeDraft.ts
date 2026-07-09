/**
 * 乐观节点草稿构建器（纯函数）。
 *
 * 依据节点注册表中的 `NodeDefinition`，在客户端本地构造一份与后端
 * `generate_initial_pins` + `PinInstanceDTO::from_pin_with_context(resolved=None)`
 * 等价的初始 NodeData + PinData，并生成客户端 id（uuid）。这样新建节点可以先于
 * 后端往返立即渲染；后端权威数据随后通过 NodeCreated 事件回传，由 handler 用相同
 * id 对齐覆盖。
 *
 * 注意：仅产出定义中的静态（Fixed）与基础可重复（Repeatable min_count）pin；
 * 由 schema 派生的动态 pin（DerivedFromInput）仍由后端在创建后通过 NodePinsUpdated
 * 补齐，故此处不生成。
 */

import type { NodeData, PinData } from '@/shared/types';
import type { DataType } from '@/shared/types/domain/dataType';
import { dataTypeDisplay } from '@/shared/types/domain/dataType';
import type {
  NodeDefinition,
  PinDataTypeDefinition,
  PinDefinitionDTO,
} from '@/shared/types/domain/node';
import type { NodeSpawnParams } from '@/shared/types/dto/nodeInstanceParams';
import {
  resolveEffectiveDefinition,
  type ResolveEffectiveOptions,
} from '@/features/domain/nodeDefinition';

export type CreateNodeDraftParams = NodeSpawnParams & ResolveEffectiveOptions;

export interface NodeDraft {
  node: NodeData;
  pins: PinData[];
  effectiveDefinition: NodeDefinition;
}

/** 对齐后端 `data_type_to_pin_type`：容器类型递归到内部类型。 */
function dataTypeToPinType(dt: DataType): string {
  switch (dt.kind) {
    case 'Boolean':
      return 'bool';
    case 'Int64':
      return 'Int64';
    case 'Float64':
      return 'Float64';
    case 'String':
      return 'string';
    case 'Date':
      return 'date';
    case 'Datetime':
      return 'datetime';
    case 'Time':
      return 'time';
    case 'Categorical':
      return 'categorical';
    case 'Object':
      return 'object';
    case 'Any':
      return 'any';
    case 'DataFrame':
      return 'dataframe';
    case 'Array':
      return dataTypeToPinType(dt.inner);
    case 'DataSeries':
      return dataTypeToPinType(dt.inner);
    case 'Struct':
      return 'struct';
    case 'OneOf':
      return 'oneof';
  }
}

/** 对齐后端 `data_type_to_container`。 */
function dataTypeToContainer(dt: DataType): string | undefined {
  if (dt.kind === 'Array') return 'array';
  if (dt.kind === 'DataSeries') return 'dataseries';
  return undefined;
}

function concreteDataType(def: PinDataTypeDefinition | null): DataType | undefined {
  if (def && typeof def === 'object' && 'Concrete' in def) return def.Concrete;
  return undefined;
}

/** 对齐后端 `generate_slot_name`。 */
function generateSlotName(prefix: string, index: number): string {
  if (!prefix) {
    return index < 26 ? String.fromCharCode(65 + index) : `Pin ${index}`;
  }
  return `${prefix} ${index + 1}`;
}

/** 将一个 pin 定义（含可选名称覆盖）转为乐观 PinData。 */
function pinFromDefinition(
  def: PinDefinitionDTO,
  nodeId: string,
  nameOverride: string | undefined,
): PinData {
  const isExec = def.kind === 'Exec';
  const dt = isExec ? undefined : concreteDataType(def.dataType);

  const type = isExec ? 'exec' : dt ? dataTypeToPinType(dt) : 'object';

  return {
    id: crypto.randomUUID(),
    nodeId,
    name: nameOverride ?? def.name,
    type,
    direction: def.direction,
    containerType: dt ? dataTypeToContainer(dt) : undefined,
    typeDisplay: dt ? dataTypeDisplay(dt) : undefined,
    dataType: dt,
    optional: def.optional ?? false,
  };
}

/** 依据定义生成初始 pin 列表（对齐后端 `generate_initial_pins`）。 */
function buildInitialPins(definition: NodeDefinition, nodeId: string): PinData[] {
  const pins: PinData[] = [];
  for (const slot of definition.pinSlots) {
    if (slot.slotKind === 'fixed') {
      pins.push(pinFromDefinition(slot.pin, nodeId, undefined));
    } else if (slot.slotKind === 'repeatable') {
      for (let i = 0; i < slot.minCount; i++) {
        pins.push(
          pinFromDefinition(slot.template, nodeId, generateSlotName(slot.namePrefix, i)),
        );
      }
    }
    // derivedFromInput: 运行时由后端补齐，乐观阶段不生成
  }
  return pins;
}

/**
 * 构建乐观节点草稿（节点 + 初始 pin），id 在内部生成。
 */
export function buildNodeDraft(
  graphPath: string,
  nodeType: string,
  definition: NodeDefinition,
  x: number,
  y: number,
  params?: CreateNodeDraftParams,
): NodeDraft {
  const nodeId = crypto.randomUUID();
  const effectiveDefinition = resolveEffectiveDefinition(definition, params);
  const pins = buildInitialPins(effectiveDefinition, nodeId);

  const inputs = pins.filter((p) => p.direction === 'input').map((p) => p.id);
  const outputs = pins.filter((p) => p.direction === 'output').map((p) => p.id);

  const paramsKind: NodeData['paramsKind'] = params?.variableId
    ? 'variable'
    : params?.subGraphPath
      ? 'subGraph'
      : params?.dataframeId
        ? 'dataFrame'
        : 'none';

  const node: NodeData = {
    id: nodeId,
    graphPath,
    nodeType,
    category: definition.category ?? [],
    title: definition.name ?? nodeType,
    inputs,
    outputs,
    uiStyle: definition.nodeMetadata?.uiStyle ?? 'default',
    position: { x, y },
    paramsKind,
    variableId: params?.variableId,
    variableName: params?.variableName,
    variableType: params?.variableType,
    subGraphPath: params?.subGraphPath,
    dataframeId: params?.dataframeId,
  };

  return { node, pins, effectiveDefinition };
}
