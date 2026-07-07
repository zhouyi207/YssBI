import type { NodeDefinition, Pin, Variable, FunctionSignaturePin } from '@/shared/types/domain';
import {
  isShellNodeDefinition,
  nodeDefinitionAllowedInGraphKind,
  variableVisibleInGraph,
} from '@/shared/types/domain';
import { isNodeCompatibleWithPin, pinAcceptsType, buildPinDataType } from '@/shared/utils/pinCompatibility';
import type { FunctionCatalogEntry } from '@/features/core/editor/hooks/useFunctionCatalog';
import type { NodeCatalogItem } from './types';

function signaturePinToDataType(pin: FunctionSignaturePin) {
  return buildPinDataType({
    id: pin.id,
    name: pin.name,
    type: pin.type,
    containerType: pin.containerType,
  });
}

export function buildContextualCatalogItems(options: {
  definitions: NodeDefinition[];
  filterPin?: Pin | null;
  variables?: Record<string, Variable>;
  functions?: Record<string, FunctionCatalogEntry>;
  graphKind?: 'event' | 'function';
  graphId?: string;
}): NodeCatalogItem[] {
  const { definitions, filterPin, variables = {}, functions = {}, graphKind, graphId } = options;
  const items: NodeCatalogItem[] = [];

  definitions.forEach((node) => {
    if (['Variables:Get Variable', 'Variables:Set Variable', 'Functions:Call Function'].includes(node.nodeType)) {
      return;
    }
    if (isShellNodeDefinition(node)) return;
    if (!nodeDefinitionAllowedInGraphKind(node, graphKind)) return;
    if (filterPin && !isNodeCompatibleWithPin(node, filterPin)) return;
    items.push({ nodeType: node.nodeType, title: node.name, category: node.category ?? [] });
  });

  Object.values(variables).forEach((v) => {
    if (!v?.name || !v?.id) return;
    if (!variableVisibleInGraph(v.scope, graphId, graphKind)) return;
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
      const signaturePins =
        filterPin.direction === 'input' ? sub.functionInputs : sub.functionOutputs;
      const hasCompatible = signaturePins.some(
        (p) => p.type !== 'exec' && pinAcceptsType(filterPin, signaturePinToDataType(p)),
      );
      if (!hasCompatible) return;
    }
    items.push({
      nodeType: 'Functions:Call Function',
      title: sub.name,
      category: ['Functions'],
      overrides: { subGraphId: sub.id, title: sub.name },
    });
  });

  return items;
}
