import { DRAG_TYPES } from '@/features/core/dnd';
import type { NodeCatalogItem } from './types';

export function buildNodeTemplateDragData(item: NodeCatalogItem) {
  return {
    type: DRAG_TYPES.NODE_TEMPLATE,
    template: {
      title: item.title,
      nodeType: item.nodeType,
      ...(item.overrides?.variableId
        ? {
            variableId: item.overrides.variableId,
            variableName: item.title.replace(/^Get |^Set /, ''),
            category: 'Variable',
          }
        : {}),
      ...(item.overrides?.subGraphId
        ? {
            subGraphId: item.overrides.subGraphId,
            subName: item.title.replace(/^Call /, ''),
            category: 'Functions',
          }
        : {}),
    },
  };
}
