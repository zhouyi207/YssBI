import type { CommandHandler, GraphDraftCommandResult } from "../types";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface AddPortInstanceArgs {
  nodeId: string;
  templateKey: string;
}

export const addPortInstanceCommand: CommandHandler<AddPortInstanceArgs, GraphDraftCommandResult> =
  {
    execute(graphPath, args) {
      return executeGraphDraftIntent(graphPath, {
        type: "addPortInstance",
        payload: { nodeId: args.nodeId, templateKey: args.templateKey, order: null },
      });
    },
  };

export interface RemovePortInstanceArgs {
  address: Extract<PortAddressDto, { kind: "instance" }>;
}

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
