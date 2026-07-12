import { DRAG_TYPES, type NodeTemplateDragData } from '@/features/core/dnd';
import {
  catalogItemNodeSpawnTemplate,
} from '@/features/core/dnd/nodeSpawnTemplate';
import type { NodeCatalogItem } from './types';

export function buildNodeTemplateDragData(item: NodeCatalogItem): NodeTemplateDragData {
  return {
    type: DRAG_TYPES.NODE_TEMPLATE,
    template: catalogItemNodeSpawnTemplate(item),
  };
}
