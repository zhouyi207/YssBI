import type { Graph, NodeDefinition, Pin, Variable } from '@/shared/types/domain';
import { isNodeCompatibleWithPin, pinAcceptsType, buildPinDataType } from '@/shared/utils/pinCompatibility';
import type { NodeCatalogItem } from './types';

export function buildContextualCatalogItems(options: {
  definitions: NodeDefinition[];
  filterPin?: Pin | null;
  variables?: Record<string, Variable>;
  functions?: Record<string, Graph>;
}): NodeCatalogItem[] {
  const { definitions, filterPin, variables = {}, functions = {} } = options;
  const items: NodeCatalogItem[] = [];

  definitions.forEach((node) => {
    if (['Variables:Get Variable', 'Variables:Set Variable', 'Functions:Call Function'].includes(node.nodeType)) {
      return;
    }
    if (filterPin && !isNodeCompatibleWithPin(node, filterPin)) return;
    items.push({ nodeType: node.nodeType, title: node.name, category: node.category ?? [] });
  });

  Object.values(variables).forEach((v) => {
    if (!v?.name || !v?.id) return;
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

  Object.values(functions).forEach((sub) => {
    if (!sub?.name || !sub?.id) return;
    if (filterPin && filterPin.type !== 'exec') {
      const targetPins = filterPin.direction === 'input' ? sub.outputs : sub.inputs;
      const hasCompatible = (targetPins ?? []).some(
        (p: Pin) => p.type !== 'exec' && pinAcceptsType(filterPin, buildPinDataType(p)),
      );
      if (!hasCompatible) return;
    }
    items.push({
      nodeType: 'Functions:Call Function',
      title: `Call ${sub.name}`,
      category: ['Functions'],
      overrides: { subGraphId: sub.id, title: sub.name },
    });
  });

  return items;
}