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

export const DEFAULT_EXECUTION_DEMAND: ExecutionDemandDto = { type: 'default' };
