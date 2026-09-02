import {
  executeCommand,
  executeCommandOutcome,
  type CommandInvocation,
  type GraphDraftCommandInvocation,
  type GraphDraftCommandType,
} from "./commandExecutor";
import type { AvailableCommandType } from "./commands/registryTypes";

type Equal<TLeft, TRight> =
  (<T>() => T extends TLeft ? 1 : 2) extends <T>() => T extends TRight ? 1 : 2 ? true : false;
type Assert<T extends true> = T;

type GraphCommandsAreDerivedFromRegistry = Assert<
  Equal<GraphDraftCommandType, AvailableCommandType>
>;

void (null as GraphCommandsAreDerivedFromRegistry | null);

function assertCommandCallTypes(): void {
  executeCommandOutcome("events/main.yssbi-event", "ConnectPins", {
    pinA: "pin-a",
    pinB: "pin-b",
  });
  executeCommand("events/main.yssbi-event", "InsertReroute", {
    connectionId: "edge-1",
    position: { x: 1, y: 2 },
  });

  executeCommandOutcome("events/main.yssbi-event", "ConnectPins", {
    // @ts-expect-error ConnectPins cannot receive MoveConnections arguments.
    sourcePinId: "pin-a",
    targetPinId: "pin-b",
  });
  // @ts-expect-error InsertReroute requires position.
  executeCommand("events/main.yssbi-event", "InsertReroute", { connectionId: "edge-1" });

  const invocation: GraphDraftCommandInvocation =
    Math.random() > 0.5
      ? ["ConnectPins", { pinA: "pin-a", pinB: "pin-b" }]
      : ["MoveConnections", { sourcePinId: "pin-a", targetPinId: "pin-b" }];
  executeCommandOutcome("events/main.yssbi-event", ...invocation);

  const anyInvocation: CommandInvocation = invocation;
  executeCommand("events/main.yssbi-event", ...anyInvocation);
}

void assertCommandCallTypes;
