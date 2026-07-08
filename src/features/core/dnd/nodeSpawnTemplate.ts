import { CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';
import type { NodeCatalogItem } from '@/features/domain/nodeCatalog/types';
import type { NodeSpawnTemplate } from './dndContracts';

export function variableNodeSpawnTemplate(
  variableId: string,
  variableName: string,
): NodeSpawnTemplate {
  return {
    title: variableName,
    nodeType: 'Variables:Get Variable',
    category: 'Variable',
    variableId,
    variableName,
  };
}

export function dataFrameNodeSpawnTemplate(
  dataframeId: string,
  name: string,
): NodeSpawnTemplate {
  return {
    title: name,
    nodeType: 'Data:Get DataFrame',
    category: 'Data',
    variableId: dataframeId,
    variableName: name,
  };
}

export function functionCallNodeSpawnTemplate(
  subGraphId: string,
  name: string,
): NodeSpawnTemplate {
  return {
    title: name,
    nodeType: CALL_FUNCTION_NODE_TYPE,
    category: 'Functions',
    subGraphId,
  };
}

export function catalogItemNodeSpawnTemplate(item: NodeCatalogItem): NodeSpawnTemplate {
  const base: NodeSpawnTemplate = {
    title: item.title,
    nodeType: item.nodeType,
  };

  if (item.overrides?.variableId) {
    return {
      ...base,
      variableId: item.overrides.variableId,
      variableName: item.title.replace(/^Get |^Set /, ''),
      category: 'Variable',
    };
  }

  if (item.overrides?.subGraphId) {
    return {
      ...base,
      subGraphId: item.overrides.subGraphId,
      category: 'Functions',
    };
  }

  return base;
}
