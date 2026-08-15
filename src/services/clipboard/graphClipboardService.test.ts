import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ClipboardSubgraphDto } from '@/shared/types/dto/clipboardSubgraph';

const pluginReadText = vi.hoisted(() => vi.fn<() => Promise<string>>());
const pluginWriteText = vi.hoisted(() => vi.fn<(text: string) => Promise<void>>());

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  readText: pluginReadText,
  writeText: pluginWriteText,
}));
import {
  GRAPH_CLIPBOARD_FORMAT,
  GRAPH_CLIPBOARD_VERSION,
  readGraphClipboard,
  writeGraphClipboard,
} from './graphClipboardService';

const snapshot: ClipboardSubgraphDto = {
  schemaVersion: 1,
  nodes: [{
    localId: 'node/0',
    creation: { kind: 'static', nodeTypeId: 'yssbi.constant.int64' },
    parameters: { value: 42 },
    userLabel: null,
    relativePosition: { x: 0, y: 0 },
  }],
  portBindings: [],
  inputStates: [],
  connections: [],
};

function installClipboard(options: {
  readText?: () => Promise<string>;
  writeText?: (text: string) => Promise<void>;
}) {
  pluginReadText.mockImplementation(options.readText ?? (async () => ''));
  pluginWriteText.mockImplementation(options.writeText ?? (async () => undefined));
  return { readText: pluginReadText, writeText: pluginWriteText };
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

describe('graphClipboardService', () => {
  it('writes one fixed-format JSON envelope and awaits the write', async () => {
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
      format: 'application/vnd.yssbi.clipboard-subgraph+json',
      version: 1,
      snapshot,
    });

    releaseWrite?.();
    await write;
  });

  it('reads and parses a valid envelope', async () => {
    installClipboard({ readText: async () => envelope() });

    await expect(readGraphClipboard()).resolves.toEqual(snapshot);
  });

  it('rejects non-JSON clipboard text with an explicit error', async () => {
    installClipboard({ readText: async () => 'ordinary clipboard text' });

    await expect(readGraphClipboard()).rejects.toMatchObject({
      name: 'GraphClipboardError',
      code: 'invalid_json',
    });
  });

  it('rejects a foreign clipboard format', async () => {
    installClipboard({ readText: async () => envelope({ format: 'text/plain' }) });

    await expect(readGraphClipboard()).rejects.toMatchObject({
      code: 'unsupported_format',
    });
  });

  it('rejects a wrong envelope version', async () => {
    installClipboard({ readText: async () => envelope({ version: 2 }) });

    await expect(readGraphClipboard()).rejects.toMatchObject({
      code: 'unsupported_version',
    });
  });

  it('rejects envelopes with missing or extra keys', async () => {
    const missingSnapshot = JSON.stringify({
      format: GRAPH_CLIPBOARD_FORMAT,
      version: GRAPH_CLIPBOARD_VERSION,
    });
    const clipboard = installClipboard({ readText: async () => missingSnapshot });

    await expect(readGraphClipboard()).rejects.toMatchObject({ code: 'invalid_envelope' });

    clipboard.readText.mockResolvedValue(envelope({ extra: true }));
    await expect(readGraphClipboard()).rejects.toMatchObject({ code: 'invalid_envelope' });
  });

  it('reuses the clipboard snapshot wire parser', async () => {
    installClipboard({
      readText: async () => envelope({
        snapshot: { ...snapshot, schemaVersion: 2 },
      }),
    });

    await expect(readGraphClipboard()).rejects.toThrow('Invalid clipboard subgraph response');
  });

  it('propagates write permission failures unchanged', async () => {
    const permissionError = new DOMException('Write denied', 'NotAllowedError');
    installClipboard({ writeText: async () => { throw permissionError; } });

    await expect(writeGraphClipboard(snapshot)).rejects.toBe(permissionError);
  });

  it('propagates read permission failures unchanged', async () => {
    const permissionError = new DOMException('Read denied', 'NotAllowedError');
    installClipboard({ readText: async () => { throw permissionError; } });

    await expect(readGraphClipboard()).rejects.toBe(permissionError);
  });


});
