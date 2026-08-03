import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import type { NodeSpawnTemplate } from './dndContracts';

function resourceNodeSpawnTemplate(
  descriptor: NodeCreationDescriptor,
  title: string,
): NodeSpawnTemplate {
  return { title, descriptor };
}

export const variableNodeSpawnTemplate = resourceNodeSpawnTemplate;
export const dataFrameNodeSpawnTemplate = resourceNodeSpawnTemplate;
export const functionCallNodeSpawnTemplate = resourceNodeSpawnTemplate;
