import type { NodeDefinition, Pin, Variable } from '@/shared/types/domain';
import {
  isShellNodeDefinition,
  nodeDefinitionAllowedInGraphKind,
  variableVisibleInGraph,
} from '@/shared/types/domain';
import {
  CALL_FUNCTION_NODE_TYPE,
  resolveEffectiveDefinition,
} from '@/features/domain/nodeDefinition';
import { isNodeCompatibleWithPin, pinAcceptsType } from '@/shared/utils/pinCompatibility';
import type { FunctionResourceView } from '@/features/core/resource/functionResourceView';
import type { NodeCatalogItem } from './types';
import { RESOURCE_SPAWNED_NODE_TYPES } from './types';

export function buildContextualCatalogItems(options: {
  definitions: NodeDefinition[];
  filterPin?: Pin | null;
  variables?: Record<string, Variable>;
  functions?: Record<string, FunctionResourceView>;
  graphKind?: 'event' | 'function';
  graphPath?: string;
}): NodeCatalogItem[] {
  const { definitions, filterPin, variables = {}, functions = {}, graphKind, graphPath } = options;
  const items: NodeCatalogItem[] = [];
  const callBase = definitions.find((d) => d.nodeType === CALL_FUNCTION_NODE_TYPE);

  definitions.forEach((node) => {
    if (RESOURCE_SPAWNED_NODE_TYPES.has(node.nodeType)) {
      return;
    }
    if (isShellNodeDefinition(node)) return;
    if (!nodeDefinitionAllowedInGraphKind(node, graphKind)) return;
    if (filterPin && !isNodeCompatibleWithPin(node, filterPin)) return;
    items.push({ nodeType: node.nodeType, title: node.name, category: node.category ?? [] });
  });

  Object.values(variables).forEach((v) => {
    if (!v?.name || !v?.id) return;
    if (!variableVisibleInGraph(v.scope, graphPath, graphKind)) return;
    const varName = v.name;
    const varId = v.id;
    const varType = v.dataType;

    let getCompatible = true;
    if (filterPin) {
      if (filterPin.direction === 'output') getCompatible = false;
      else getCompatible = pinAcceptsType(filterPin, varType);
    }
    if (getCompatible) {
      items.push({
        nodeType: 'Variables:Get Variable',
        title: `Get ${varName}`,
        category: ['Variables'],
        overrides: { title: 'Get Variable', variableId: varId },
      });
    }

    let setCompatible = true;
    if (filterPin) {
      if (filterPin.direction === 'input') setCompatible = false;
      else setCompatible = pinAcceptsType(filterPin, varType);
    }
    if (setCompatible) {
      items.push({
        nodeType: 'Variables:Set Variable',
        title: `Set ${varName}`,
        category: ['Variables'],
        overrides: { title: 'Set Variable', variableId: varId },
      });
    }
  });

  if (callBase) {
    Object.values(functions).forEach((sub) => {
      if (!sub?.name || !sub?.id) return;
      const effective = resolveEffectiveDefinition(callBase, {
        subGraphPath: sub.id,
        functionInputs: sub.functionInputs,
        functionOutputs: sub.functionOutputs,
      });
      if (filterPin && !isNodeCompatibleWithPin(effective, filterPin)) return;

      items.push({
        nodeType: CALL_FUNCTION_NODE_TYPE,
        title: sub.name,
        category: ['Functions'],
        overrides: { subGraphPath: sub.id, title: sub.name },
      });
    });
  }

  return items;
}
