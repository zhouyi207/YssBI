import { describe, expect, it } from "vitest";
import { derivePinConnectionView } from "./pinLinks";

describe("pinLinks", () => {
  it("derives connected state from pinConnections ids", () => {
    expect(derivePinConnectionView(undefined)).toEqual({
      connected: false,
      linkCount: 0,
      connectionIds: [],
    });
    expect(derivePinConnectionView(["pin-a->pin-b"])).toEqual({
      connected: true,
      linkCount: 1,
      connectionIds: ["pin-a->pin-b"],
    });
  });
});
