import { logger } from "@/utils/appLogger";

export const EDITOR_MUTATION_CAPABILITIES = {
  createStaticNodes: true,
  catalogDescriptors: true,
  resourceBoundDescriptors: true,
  contextualCompatibility: true,
  nodeDocumentation: true,
  duplicateNodes: true,
  pasteNodes: true,
} as const;

export const NODE_CREATION_UNAVAILABLE_MESSAGE =
  'Node creation is unavailable until stable catalog descriptors are available';

export const RESOURCE_CATALOG_REFRESH_MESSAGE =
  'Resource catalog is stale. Refreshing before node creation.';

export function notifyNodeCreationUnavailable(): void {
  logger.notify.info(NODE_CREATION_UNAVAILABLE_MESSAGE, "UI");
}
