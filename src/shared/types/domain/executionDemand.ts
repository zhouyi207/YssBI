import type { PortAddressDto } from "@/shared/types/domain/editorProjection";

export interface GraphOutputRefDto {
  graphPath: string;
  port: PortAddressDto;
}

export type ExecutionDemandDto =
  | { type: "default" }
  | { type: "node"; nodeId: string; mode: "currentInputs" | "dependencies" }
  | {
      type: "outputs";
      outputs: GraphOutputRefDto[];
      includeDefaultResults: boolean;
      reuseInputs: boolean;
    };

export const EXECUTION_DEMAND_TYPES = {
  default: true,
  node: true,
  outputs: true,
} as const satisfies Record<ExecutionDemandDto["type"], true>;
