import { afterEach, describe, expect, it, vi } from "vitest";
import type { ClipboardSubgraphDto } from "@/shared/types/dto/clipboardSubgraph";

const platformReadText = vi.hoisted(() => vi.fn<() => Promise<unknown>>());
const platformWriteText = vi.hoisted(() => vi.fn<(text: string) => Promise<unknown>>());

vi.mock("@/services/platform/clipboard", () => ({
  readClipboardText: platformReadText,
  writeClipboardText: platformWriteText,
}));
import {
  GRAPH_CLIPBOARD_FORMAT,
  GRAPH_CLIPBOARD_VERSION,
  readGraphClipboard,
  writeGraphClipboard,
} from "./graphClipboardService";

const snapshot: ClipboardSubgraphDto = {
  schemaVersion: 1,
  nodes: [
    {
      localId: "node/0",
      creation: { kind: "static", nodeTypeId: "yssbi.constant.int64" },
      parameters: { value: 42 },
      userLabel: null,
      relativePosition: { x: 0, y: 0 },
    },
  ],
  portBindings: [],
  inputStates: [],
  connections: [],
};

function installClipboard(
  options: {
    readText?: () => Promise<string>;
    writeText?: (text: string) => Promise<void>;
  } = {},
) {
  platformReadText.mockImplementation(async () => ({
    ok: true,
    value: await (options.readText ?? (async () => ""))(),
  }));
  platformWriteText.mockImplementation(async (text) => {
    await (options.writeText ?? (async () => undefined))(text);
    return { ok: true, value: undefined };
  });
  return { readText: platformReadText, writeText: platformWriteText };
}

function envelope(overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    format: GRAPH_CLIPBOARD_FORMAT,
    version: GRAPH_CLIPBOARD_VERSION,
    snapshot,
    ...overrides,
  });
}

afterEach(() => {
  vi.clearAllMocks();
});

describe("graphClipboardService", () => {
  it("writes one fixed-format JSON envelope and awaits the write", async () => {
    let releaseWrite: (() => void) | undefined;
    const pendingWrite = new Promise<void>((resolve) => {
      releaseWrite = resolve;
    });
    const { writeText } = installClipboard({ writeText: () => pendingWrite });

    let settled = false;
    const write = writeGraphClipboard(snapshot).then(() => {
      settled = true;
    });
    await Promise.resolve();

    expect(settled).toBe(false);
    expect(writeText).toHaveBeenCalledOnce();
    expect(JSON.parse(writeText.mock.calls[0][0])).toEqual({
      format: "application/vnd.yssbi.clipboard-subgraph+json",
      version: 1,
      snapshot,
    });

    releaseWrite?.();
    await write;
  });

  it("reads and parses a valid envelope", async () => {
    installClipboard({ readText: async () => envelope() });

    await expect(readGraphClipboard()).resolves.toEqual(snapshot);
  });

  it("rejects non-JSON clipboard text with an explicit error", async () => {
    installClipboard({ readText: async () => "ordinary clipboard text" });

    await expect(readGraphClipboard()).rejects.toMatchObject({
      name: "GraphClipboardError",
      code: "invalid_json",
    });
  });

  it("rejects a foreign clipboard format", async () => {
    installClipboard({ readText: async () => envelope({ format: "text/plain" }) });

    await expect(readGraphClipboard()).rejects.toMatchObject({
      code: "unsupported_format",
    });
  });

  it("rejects a wrong envelope version", async () => {
    installClipboard({ readText: async () => envelope({ version: 2 }) });

    await expect(readGraphClipboard()).rejects.toMatchObject({
      code: "unsupported_version",
    });
  });

  it("rejects envelopes with missing or extra keys", async () => {
    const missingSnapshot = JSON.stringify({
      format: GRAPH_CLIPBOARD_FORMAT,
      version: GRAPH_CLIPBOARD_VERSION,
    });
    const clipboard = installClipboard({ readText: async () => missingSnapshot });

    await expect(readGraphClipboard()).rejects.toMatchObject({ code: "invalid_envelope" });

    clipboard.readText.mockResolvedValue({ ok: true, value: envelope({ extra: true }) });
    await expect(readGraphClipboard()).rejects.toMatchObject({ code: "invalid_envelope" });
  });

  it("reuses the clipboard snapshot wire parser", async () => {
    installClipboard({
      readText: async () =>
        envelope({
          snapshot: { ...snapshot, schemaVersion: 2 },
        }),
    });

    await expect(readGraphClipboard()).rejects.toThrow("Invalid clipboard subgraph response");
  });

  it("propagates write permission failures unchanged", async () => {
    installClipboard();
    platformWriteText.mockResolvedValue({
      ok: false,
      failure: { operation: "writeClipboardText", code: "operationFailed" },
    });

    await expect(writeGraphClipboard(snapshot)).rejects.toMatchObject({ code: "platform" });
  });

  it("propagates read permission failures unchanged", async () => {
    installClipboard();
    platformReadText.mockResolvedValue({
      ok: false,
      failure: { operation: "readClipboardText", code: "operationFailed" },
    });

    await expect(readGraphClipboard()).rejects.toMatchObject({ code: "platform" });
  });
});
