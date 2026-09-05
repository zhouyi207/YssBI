import type { CommandHandler, GraphDraftCommandResult } from "../types";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import type { PortPlacementDto } from "@/shared/types/domain/editorMutation";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface AddPortInstanceArgs {
  nodeId: string;
  templateKey: string;
  placement?: PortPlacementDto;
}

export const addPortInstanceCommand: CommandHandler<AddPortInstanceArgs, GraphDraftCommandResult> =
  {
    execute(graphPath, args) {
      return executeGraphDraftIntent(graphPath, {
        type: "addPortInstance",
        payload: {
          nodeId: args.nodeId,
          templateKey: args.templateKey,
          placement: args.placement ?? { kind: "append" },
        },
      });
    },
  };

export interface RemovePortInstanceArgs {
  address: Extract<PortAddressDto, { kind: "instance" }>;
}

export interface MovePortInstanceArgs extends RemovePortInstanceArgs {
  placement: PortPlacementDto;
}

export const movePortInstanceCommand: CommandHandler<
  MovePortInstanceArgs,
  GraphDraftCommandResult
> = {
  execute(graphPath, args) {
    return executeGraphDraftIntent(graphPath, { type: "movePortInstance", payload: args });
  },
};

export const removePortInstanceCommand: CommandHandler<
  RemovePortInstanceArgs,
  GraphDraftCommandResult
> = {
  execute(graphPath, args) {
    return executeGraphDraftIntent(graphPath, {
      type: "removePortInstance",
      payload: { address: args.address },
    });
  },
};
