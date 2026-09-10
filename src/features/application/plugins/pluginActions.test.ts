import { beforeEach, expect, it, vi } from "vitest";
import type { TFunction } from "i18next";
import { installLocalPlugin } from "./pluginActions";

const mocks = vi.hoisted(() => ({ inspect: vi.fn(), install: vi.fn(), confirm: vi.fn() }));
vi.mock("@/services/plugins/pluginService", async (original) => ({
  ...(await original<typeof import("@/services/plugins/pluginService")>()),
  pluginService: mocks,
}));
vi.mock("@/features/core/ui/ui", () => ({ ui: { confirm: mocks.confirm } }));
vi.mock("@/services/platform/pathDialog", () => ({
  openPathDialog: async () => ({ ok: true, value: "plugin.yssplugin" }),
  savePathDialog: vi.fn(),
}));
vi.mock("@/services/platform/opener", () => ({ revealPath: vi.fn() }));
const t = ((key: string) => key) as TFunction;

beforeEach(() => {
  vi.clearAllMocks();
  mocks.inspect.mockResolvedValue({
    manifest: { name: "Example", publisher: "Example", permissions: [] },
    packageDigest: "digest",
    signerKey: "new-key",
    previousSignerKey: "old-key",
  });
});

it("requires separate signer-change consent and binds approval to the inspected fingerprint", async () => {
  mocks.confirm.mockResolvedValueOnce(false);
  await installLocalPlugin(t);
  expect(mocks.install).not.toHaveBeenCalled();
  expect(mocks.confirm).toHaveBeenCalledTimes(1);

  mocks.confirm.mockResolvedValueOnce(true).mockResolvedValueOnce(true);
  await installLocalPlugin(t);
  expect(mocks.confirm.mock.calls.slice(1).map(([options]) => options.title)).toEqual([
    "plugins.signerChangeTitle",
    "plugins.trustTitle",
  ]);
  expect(mocks.install).toHaveBeenCalledWith(
    "plugin.yssplugin",
    "digest",
    expect.stringMatching(/^op-\d+-/),
    "old-key",
  );
});
