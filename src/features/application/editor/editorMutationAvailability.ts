import { uiStore } from '@/features/core/ui/UIStore';

export const EDITOR_MUTATION_CAPABILITIES = {
  createStaticNodes: true,
  catalogDescriptors: true,
  resourceBoundDescriptors: true,
  contextualCompatibility: true,
  nodeDocumentation: true,
  duplicateNodes: false,
  pasteNodes: false,
} as const;

export const NODE_CREATION_UNAVAILABLE_MESSAGE =
  'Node creation is unavailable until stable catalog descriptors are available';

export const RESOURCE_CATALOG_REFRESH_MESSAGE =
  'Resource catalog is stale. Refreshing before node creation.';

export function notifyNodeCreationUnavailable(): void {
  uiStore.showToast(NODE_CREATION_UNAVAILABLE_MESSAGE, 'info', 3000);
}
