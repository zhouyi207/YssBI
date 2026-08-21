import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import type { GraphOutputRefDto } from '@/shared/types/dto/result';

const part = (value: string): string => `${value.length}:${value}`;

export function portAddressKey(address: PortAddressDto): string {
  if (address.kind === 'declared') {
    return `declared:${part(address.nodeId)}${part(address.portKey)}`;
  }

  return `instance:${part(address.nodeId)}${part(address.templateKey)}${part(address.instanceId)}`;
}

export function graphOutputKey(output: GraphOutputRefDto): string {
  return `${part(output.graphPath)}:${portAddressKey(output.port)}`;
}
