import { executeGraphEdit, type GraphEditInvocation } from "./commandExecutor";

function assertCommandCallTypes(): void {
  executeGraphEdit("events/main.yssbi-event", "ConnectPins", {
    pinA: "pin-a",
    pinB: "pin-b",
  });
  executeGraphEdit("events/main.yssbi-event", "InsertReroute", {
    connectionId: "edge-1",
    position: { x: 1, y: 2 },
  });

  executeGraphEdit("events/main.yssbi-event", "ConnectPins", {
    // @ts-expect-error ConnectPins cannot receive MoveConnections arguments.
    sourcePinId: "pin-a",
    targetPinId: "pin-b",
  });
  // @ts-expect-error InsertReroute requires position.
  executeGraphEdit("events/main.yssbi-event", "InsertReroute", { connectionId: "edge-1" });

  const invocation: GraphEditInvocation =
    Math.random() > 0.5
      ? ["ConnectPins", { pinA: "pin-a", pinB: "pin-b" }]
      : ["MoveConnections", { sourcePinId: "pin-a", targetPinId: "pin-b" }];
  executeGraphEdit("events/main.yssbi-event", ...invocation);

  const anyInvocation: GraphEditInvocation = invocation;
  executeGraphEdit("events/main.yssbi-event", ...anyInvocation);
}

void assertCommandCallTypes;
