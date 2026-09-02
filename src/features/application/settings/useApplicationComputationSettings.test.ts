// @vitest-environment happy-dom

import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ApplicationSettingsMutationReceiptDto,
  ApplicationSettingsSnapshotDto,
} from "@/shared/types/dto/applicationSettings";
import { ApplicationSettingsService } from "@/services/settings/applicationSettingsService";
import { useApplicationComputationSettings } from "./useApplicationComputationSettings";

vi.mock("@/services/settings/applicationSettingsService", () => ({
  ApplicationSettingsService: {
    get: vi.fn(),
    update: vi.fn(),
  },
}));

function snapshot(
  overrides: Partial<ApplicationSettingsSnapshotDto> = {},
): ApplicationSettingsSnapshotDto {
  return {
    settingsRevision: 3,
    settings: {
      computation: {
        numeric: { tolerance: { absolute: 1e-12, relative: 1e-9 } },
        missingValues: { statistics: "listwise" },
      },
    },
    ...overrides,
  };
}

describe("useApplicationComputationSettings", () => {
  let host: HTMLDivElement;
  let root: Root;
  let current: ReturnType<typeof useApplicationComputationSettings> | undefined;

  function Harness() {
    current = useApplicationComputationSettings();
    return null;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ApplicationSettingsService.get).mockResolvedValue(snapshot());
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  async function render(): Promise<void> {
    await act(async () => {
      root.render(createElement(Harness));
      await Promise.resolve();
    });
  }

  it("loads one global snapshot without depending on an active project", async () => {
    await render();

    expect(ApplicationSettingsService.get).toHaveBeenCalledOnce();
    expect(current?.enabled).toBe(true);
    expect(current?.confirmed?.settingsRevision).toBe(3);
    expect(current?.draft).toEqual({
      absolute: "1e-12",
      relative: "1e-9",
      statistics: "listwise",
    });
  });

  it("validates the global tolerance pair", async () => {
    await render();

    act(() => current?.setDraft({ absolute: "0", relative: "0" }));
    expect(current?.validationError).toMatch(/cannot both be zero/i);
    act(() => current?.setDraft({ absolute: "1e-8", relative: "0" }));
    expect(current?.validationError).toBeNull();
  });

  it("applies a revisioned global settings update", async () => {
    await render();
    act(() => current?.setDraft({ absolute: "1e-8", statistics: "reject" }));
    const result: ApplicationSettingsMutationReceiptDto = {
      ...snapshot({ settingsRevision: 4 }),
      operationId: "operation-a",
      settings: {
        computation: {
          numeric: { tolerance: { absolute: 1e-8, relative: 1e-9 } },
          missingValues: { statistics: "reject" },
        },
      },
    };
    vi.mocked(ApplicationSettingsService.update).mockImplementation(async (request) => ({
      ...result,
      operationId: request.operationId,
    }));

    await act(async () => {
      await current?.apply();
    });

    expect(ApplicationSettingsService.update).toHaveBeenCalledWith(
      expect.objectContaining({
        expectedRevision: 3,
        operationId: expect.any(String),
        settings: { computation: result.settings.computation },
      }),
    );
    expect(current?.confirmed?.settingsRevision).toBe(4);
    expect(current?.isDirty).toBe(false);
  });
});
