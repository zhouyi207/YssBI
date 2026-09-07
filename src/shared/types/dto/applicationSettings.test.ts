import { describe, expect, it } from "vitest";

import {
  parseApplicationSettingsMutationReceipt,
  parseApplicationSettingsSnapshot,
} from "./applicationSettings";

describe("application settings wire", () => {
  it("accepts the current preference and rejects obsolete or invalid computation fields", () => {
    const snapshot = {
      settingsRevision: 3,
      settings: { computation: { missingValues: { statistics: "reject" } } },
    };
    expect(parseApplicationSettingsSnapshot(snapshot)).toEqual(snapshot);
    const receipt = { ...snapshot, operationId: "operation-a" };
    expect(parseApplicationSettingsMutationReceipt(receipt)).toEqual(receipt);

    const obsolete = {
      ...snapshot,
      settings: {
        computation: {
          ...snapshot.settings.computation,
          numeric: { tolerance: { absolute: 1e-12, relative: 1e-9 } },
        },
      },
    };
    expect(() => parseApplicationSettingsSnapshot(obsolete)).toThrow();
    expect(() =>
      parseApplicationSettingsMutationReceipt({ ...obsolete, operationId: "operation-a" }),
    ).toThrow();
    expect(() =>
      parseApplicationSettingsSnapshot({
        ...snapshot,
        settings: { computation: { missingValues: { statistics: "unknown" } } },
      }),
    ).toThrow();
  });
});
