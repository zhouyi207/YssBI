import { uiStore } from '@/features/core/ui/UIStore';

export const EDITOR_MUTATION_CAPABILITIES = {
  createStaticNodes: true,
  catalogDescriptors: true,
  resourceBoundDescriptors: false,
  contextualCompatibility: false,
  nodeDocumentation: false,
  duplicateNodes: false,
  pasteNodes: false,
} as const;

export const NODE_CREATION_UNAVAILABLE_MESSAGE =
  'Node creation is unavailable until stable catalog descriptors are available';

export const NODE_CATALOG_UNAVAILABLE_MESSAGE =
  'Node catalog and documentation are unavailable until stable catalog descriptors are available';

export function notifyNodeCreationUnavailable(): void {
  uiStore.showToast(NODE_CREATION_UNAVAILABLE_MESSAGE, 'info', 3000);
}
