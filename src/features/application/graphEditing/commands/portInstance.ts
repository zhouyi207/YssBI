import type { CommandHandler, GraphEditOutcome } from "../types";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import type { PortPlacementDto } from "@/shared/types/domain/editorMutation";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface AddPortInstanceArgs {
  nodeId: string;
  templateKey: string;
  placement?: PortPlacementDto;
}

export const addPortInstanceCommand: CommandHandler<AddPortInstanceArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "addPortInstance",
        payload: {
          nodeId: args.nodeId,
          templateKey: args.templateKey,
          placement: args.placement ?? { kind: "append" },
        },
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

export const movePortInstanceCommand: CommandHandler<MovePortInstanceArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    return applyGraphDraftMutation({
      graphPath,
      mutation: { type: "movePortInstance", payload: args },
    });
  },
};

export const removePortInstanceCommand: CommandHandler<RemovePortInstanceArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "removePortInstance",
        payload: { address: args.address },
      },
    });
  },
};
