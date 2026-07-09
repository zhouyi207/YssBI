/**
 * 节点实例参数 — 对齐 Rust `graph::node::NodeInstanceParams`（`#[serde(tag = "paramsKind")]`）
 * 创建 / undo / 图 DTO 共用 tagged union，避免 command 层静默丢字段。
 */

export type ParamsKind = 'none' | 'variable' | 'subGraph' | 'dataFrame';

/** 创建节点时的扁平 spawn 参数（IPC 前由 `spawnParamsToInstanceParams` 打 tag） */
export interface NodeSpawnParams {
  variableId?: string;
  variableName?: string;
  variableType?: string;
  subGraphPath?: string;
  dataframeId?: string;
}

export type NodeInstanceParamsDTO =
  | { paramsKind: 'none' }
  | {
      paramsKind: 'variable';
      variableId: string;
      variableName?: string;
      variableType?: string;
    }
  | { paramsKind: 'subGraph'; subGraphPath: string }
  | { paramsKind: 'dataFrame'; dataframeId: string };

export const NODE_INSTANCE_PARAMS_NONE: NodeInstanceParamsDTO = { paramsKind: 'none' };

export function spawnParamsToInstanceParams(
  params?: NodeSpawnParams,
): NodeInstanceParamsDTO | null {
  if (!params) return null;
  if (params.variableId) {
    return {
      paramsKind: 'variable',
      variableId: params.variableId,
      ...(params.variableName ? { variableName: params.variableName } : {}),
      ...(params.variableType ? { variableType: params.variableType } : {}),
    };
  }
  if (params.subGraphPath) {
    return { paramsKind: 'subGraph', subGraphPath: params.subGraphPath };
  }
  if (params.dataframeId) {
    return { paramsKind: 'dataFrame', dataframeId: params.dataframeId };
  }
  return NODE_INSTANCE_PARAMS_NONE;
}

/** 从 store 扁平字段还原 tagged union（undo 快照 / 导出边界） */
export function nodeSpawnFieldsToInstanceParams(
  fields: NodeSpawnParams & { paramsKind?: ParamsKind },
): NodeInstanceParamsDTO {
  const tagged = spawnParamsToInstanceParams(fields);
  if (tagged) return tagged;
  if (fields.paramsKind === 'variable' && fields.variableId) {
    return {
      paramsKind: 'variable',
      variableId: fields.variableId,
      ...(fields.variableName ? { variableName: fields.variableName } : {}),
      ...(fields.variableType ? { variableType: fields.variableType } : {}),
    };
  }
  if (fields.paramsKind === 'subGraph' && fields.subGraphPath) {
    return { paramsKind: 'subGraph', subGraphPath: fields.subGraphPath };
  }
  if (fields.paramsKind === 'dataFrame' && fields.dataframeId) {
    return { paramsKind: 'dataFrame', dataframeId: fields.dataframeId };
  }
  return NODE_INSTANCE_PARAMS_NONE;
}

/** `NodeInstanceDTO` flatten 字段 → store 扁平形态 */
export function flattenInstanceParams(
  params: NodeInstanceParamsDTO,
): NodeSpawnParams & { paramsKind: ParamsKind } {
  switch (params.paramsKind) {
    case 'variable':
      return {
        paramsKind: 'variable',
        variableId: params.variableId,
        variableName: params.variableName,
        variableType: params.variableType,
      };
    case 'subGraph':
      return { paramsKind: 'subGraph', subGraphPath: params.subGraphPath };
    case 'dataFrame':
      return { paramsKind: 'dataFrame', dataframeId: params.dataframeId };
    default:
      return { paramsKind: 'none' };
  }
}
