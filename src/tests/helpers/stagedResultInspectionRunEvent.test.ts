import { describe, expect, it } from "vitest";
import stagedResultInspectionWire from "@/tests/fixtures/node-system-contracts/staged-result-inspection-wire.json";
import {
  parseStagedResultInspectionRunEvent,
  type ResultInspectionRequestedRunEvent,
} from "./stagedResultInspectionRunEvent";

function clone<T>(value: T): T {
  return structuredClone(value);
}

describe("staged result inspection run-event parser", () => {
  it("parses the exact inspection wire and preserves opaque outer and source identities", () => {
    expect(parseStagedResultInspectionRunEvent(stagedResultInspectionWire)).toEqual(
      stagedResultInspectionWire,
    );

    const parsed = parseStagedResultInspectionRunEvent({
      ...stagedResultInspectionWire,
      run: {
        ...stagedResultInspectionWire.run,
        projectSessionId: "session with spaces",
        graphPath: "opaque outer graph identity",
        runId: "9007199254740993",
      },
      kind: {
        ...stagedResultInspectionWire.kind,
        resultId: "9007199254740995",
        source: {
          ...stagedResultInspectionWire.kind.source,
          graphPath: "opaque nested graph identity",
          nodeId: "opaque node identity",
          portAddress: "opaque port identity",
        },
      },
    });

    expect(parsed.run).toEqual({
      projectSessionId: "session with spaces",
      graphPath: "opaque outer graph identity",
      runId: "9007199254740993",
    });
    expect(parsed.kind).toEqual({
      type: "resultInspectionRequested",
      resultId: "9007199254740995",
      source: {
        graphPath: "opaque nested graph identity",
        nodeId: "opaque node identity",
        portAddress: "opaque port identity",
      },
    });
  });

  it("keeps inspection before runCompleted on the same ordered run sequence", () => {
    const orderedWire = [
      stagedResultInspectionWire,
      {
        run: stagedResultInspectionWire.run,
        kind: { type: "runCompleted" as const },
      },
    ];

    const parsed = orderedWire.map(parseStagedResultInspectionRunEvent);

    expect(parsed.map((event) => event.kind.type)).toEqual([
      "resultInspectionRequested",
      "runCompleted",
    ]);
    expect(parsed[0]).toMatchObject({ run: stagedResultInspectionWire.run });
    expect(parsed[1]).toEqual({
      run: stagedResultInspectionWire.run,
      kind: { type: "runCompleted" },
    });
  });

  it.each(["policy", "target", "message", "route", "payload"])(
    "rejects legacy %s fields on the inspection kind",
    (field) => {
      const invalid = clone(stagedResultInspectionWire) as {
        kind: Record<string, unknown>;
      };
      invalid.kind[field] = field;

      expect(() => parseStagedResultInspectionRunEvent(invalid)).toThrow(
        "Invalid staged result inspection run event",
      );
    },
  );

  it.each([
    [
      "missing run key",
      (event: Record<string, unknown>) => {
        delete event.run;
      },
    ],
    [
      "extra outer key",
      (event: Record<string, unknown>) => {
        event.extra = true;
      },
    ],
    [
      "extra source key",
      (event: Record<string, unknown>) => {
        ((event.kind as Record<string, unknown>).source as Record<string, unknown>).extra = true;
      },
    ],
    [
      "missing source key",
      (event: Record<string, unknown>) => {
        delete (event.kind as Record<string, unknown>).source;
      },
    ],
  ])("rejects non-exact staged event shape: %s", (_label, mutate) => {
    const invalid = clone(stagedResultInspectionWire) as unknown as Record<string, unknown>;
    mutate(invalid);

    expect(() => parseStagedResultInspectionRunEvent(invalid)).toThrow();
  });

  it.each([
    [
      "run projectSessionId",
      (event: Record<string, unknown>) => {
        (event.run as Record<string, unknown>).projectSessionId = "";
      },
    ],
    [
      "run graphPath",
      (event: Record<string, unknown>) => {
        (event.run as Record<string, unknown>).graphPath = "";
      },
    ],
    [
      "source graphPath",
      (event: Record<string, unknown>) => {
        ((event.kind as Record<string, unknown>).source as Record<string, unknown>).graphPath = "";
      },
    ],
    [
      "source nodeId",
      (event: Record<string, unknown>) => {
        ((event.kind as Record<string, unknown>).source as Record<string, unknown>).nodeId = "";
      },
    ],
    [
      "source portAddress",
      (event: Record<string, unknown>) => {
        ((event.kind as Record<string, unknown>).source as Record<string, unknown>).portAddress =
          "";
      },
    ],
  ])("rejects empty opaque identity: %s", (_label, mutate) => {
    const invalid = clone(stagedResultInspectionWire) as unknown as Record<string, unknown>;
    mutate(invalid);

    expect(() => parseStagedResultInspectionRunEvent(invalid)).toThrow();
  });

  it.each(["", "0", "01", "-1", "1.0", "1e2", 1, null])(
    "rejects invalid positive decimal runId %j",
    (runId) => {
      const invalid = clone(stagedResultInspectionWire) as unknown as {
        run: Record<string, unknown>;
      };
      invalid.run.runId = runId;

      expect(() => parseStagedResultInspectionRunEvent(invalid)).toThrow();
    },
  );

  it.each(["", "0", "01", "-1", "1.0", "1e2", 1, null])(
    "rejects invalid positive decimal resultId %j",
    (resultId) => {
      const invalid = clone(stagedResultInspectionWire) as unknown as {
        kind: Record<string, unknown>;
      };
      invalid.kind.resultId = resultId;

      expect(() => parseStagedResultInspectionRunEvent(invalid)).toThrow();
    },
  );

  it("accepts nullable source node and port identities without adding fields", () => {
    const value = clone(stagedResultInspectionWire) as unknown as {
      kind: ResultInspectionRequestedRunEvent;
    };
    value.kind.source.nodeId = null;
    value.kind.source.portAddress = null;

    expect(parseStagedResultInspectionRunEvent(value)).toEqual(value);
  });
});
