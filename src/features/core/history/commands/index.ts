import type { AvailableCommandType, CommandHandlerMap } from "./registryTypes";
import { moveNodesCommand } from "./moveNodes";
import { setPinValueCommand } from "./setPinValue";
import { connectPinsCommand } from "./connectPins";
import { disconnectPortCommand } from "./disconnectPin";
import { disconnectNodeCommand } from "./disconnectNode";
import { disconnectConnectionsCommand } from "./disconnectConnections";
import { insertRerouteCommand } from "./insertReroute";
import { moveConnectionsCommand } from "./moveConnections";
import { deleteNodesCommand } from "./deleteNodes";
import { duplicateSubgraphCommand } from "./duplicateSubgraph";
import { insertSubgraphCommand } from "./insertSubgraph";
import { addPortInstanceCommand, removePortInstanceCommand } from "./portInstance";

export const commandRegistry: CommandHandlerMap = {
  MoveNodes: moveNodesCommand,
  SetPinValue: setPinValueCommand,
  ConnectPins: connectPinsCommand,
  DisconnectPort: disconnectPortCommand,
  DisconnectNode: disconnectNodeCommand,
  DisconnectConnections: disconnectConnectionsCommand,
  InsertReroute: insertRerouteCommand,
  MoveConnections: moveConnectionsCommand,
  DeleteNodes: deleteNodesCommand,
  DuplicateSubgraph: duplicateSubgraphCommand,
  InsertSubgraph: insertSubgraphCommand,
  AddPortInstance: addPortInstanceCommand,
  RemovePortInstance: removePortInstanceCommand,
};

export function getCommandHandler(type: AvailableCommandType) {
  return commandRegistry[type];
}

export type { CommandHandlerMap } from "./registryTypes";
export type { MoveNodesArgs } from "./moveNodes";
export type { SetPinValueArgs } from "./setPinValue";
export type { ConnectPinsArgs } from "./connectPins";
export type { DisconnectPortArgs } from "./disconnectPin";
export type { DisconnectNodeArgs } from "./disconnectNode";
export type { DisconnectConnectionsArgs } from "./disconnectConnections";
export type { InsertRerouteArgs } from "./insertReroute";
export type { MoveConnectionsArgs } from "./moveConnections";
export type { DeleteNodesArgs } from "./deleteNodes";
export type { DuplicateSubgraphArgs } from "./duplicateSubgraph";
export type { InsertSubgraphArgs } from "./insertSubgraph";
export type { AddPortInstanceArgs, RemovePortInstanceArgs } from "./portInstance";
