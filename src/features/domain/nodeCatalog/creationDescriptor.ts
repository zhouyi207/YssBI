import {
  isNodeCreationDescriptorDto,
  type NodeCreationDescriptorDto,
} from '@/shared/types/dto/nodeCreationDescriptor';

export type NodeCreationDescriptor = NodeCreationDescriptorDto;
export const isNodeCreationDescriptor = isNodeCreationDescriptorDto;
