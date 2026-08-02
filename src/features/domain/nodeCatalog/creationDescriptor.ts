export interface StaticNodeCreationDescriptor {
  kind: 'static';
  nodeTypeId: string;
}

export type NodeCreationDescriptor = StaticNodeCreationDescriptor;

export function isNodeCreationDescriptor(value: unknown): value is NodeCreationDescriptor {
  if (typeof value !== 'object' || value === null) return false;

  const candidate = value as Record<string, unknown>;
  return candidate.kind === 'static' && typeof candidate.nodeTypeId === 'string';
}
