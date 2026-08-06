import type { PortAddressDto } from './editorProjection';

export interface GraphOutputRefDto {
  graphPath: string;
  port: PortAddressDto;
}

export type ExecutionDemandDto =
  | { type: 'default' }
  | {
      type: 'outputs';
      outputs: GraphOutputRefDto[];
      includeDefaultResults: boolean;
    };

export const EXECUTION_DEMAND_TYPES = {
  default: true,
  outputs: true,
} as const satisfies Record<ExecutionDemandDto['type'], true>;

export const DEFAULT_EXECUTION_DEMAND: ExecutionDemandDto = { type: 'default' };
