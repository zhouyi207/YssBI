import {
  isNodeCreationDescriptorDto,
  type NodeCreationDescriptorDto,
} from '@/shared/types/domain/nodeCreationDescriptor';

export type NodeCreationDescriptor = NodeCreationDescriptorDto;
export const isNodeCreationDescriptor = isNodeCreationDescriptorDto;
