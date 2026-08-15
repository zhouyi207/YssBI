import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager';
import type { ClipboardSubgraphDto } from '@/shared/types/dto/clipboardSubgraph';
import { parseClipboardSubgraphDto } from '@/shared/types/dto/clipboardSubgraphWireParser';

export const GRAPH_CLIPBOARD_FORMAT = 'application/vnd.yssbi.clipboard-subgraph+json' as const;
export const GRAPH_CLIPBOARD_VERSION = 1 as const;

export interface GraphClipboardEnvelope {
  format: typeof GRAPH_CLIPBOARD_FORMAT;
  version: typeof GRAPH_CLIPBOARD_VERSION;
  snapshot: ClipboardSubgraphDto;
}

export type GraphClipboardErrorCode =
  | 'invalid_json'
  | 'invalid_envelope'
  | 'unsupported_format'
  | 'unsupported_version';

export class GraphClipboardError extends Error {
  constructor(
    public readonly code: GraphClipboardErrorCode,
    message: string,
  ) {
    super(message);
    this.name = 'GraphClipboardError';
  }
}


function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactEnvelopeKeys(value: Record<string, unknown>): boolean {
  const keys = Object.keys(value);
  return keys.length === 3
    && keys.every((key) => ['format', 'version', 'snapshot'].includes(key));
}

export async function writeGraphClipboard(snapshot: ClipboardSubgraphDto): Promise<void> {
  const envelope: GraphClipboardEnvelope = {
    format: GRAPH_CLIPBOARD_FORMAT,
    version: GRAPH_CLIPBOARD_VERSION,
    snapshot,
  };
  await writeText(JSON.stringify(envelope));
}

export async function readGraphClipboard(): Promise<ClipboardSubgraphDto> {
  const text = await readText();
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new GraphClipboardError('invalid_json', 'Clipboard text is not valid JSON');
  }

  if (!isRecord(value) || !hasExactEnvelopeKeys(value)) {
    throw new GraphClipboardError('invalid_envelope', 'Clipboard JSON is not a graph envelope');
  }
  if (value.format !== GRAPH_CLIPBOARD_FORMAT) {
    throw new GraphClipboardError('unsupported_format', 'Clipboard graph format is unsupported');
  }
  if (value.version !== GRAPH_CLIPBOARD_VERSION) {
    throw new GraphClipboardError('unsupported_version', 'Clipboard graph version is unsupported');
  }

  return parseClipboardSubgraphDto(value.snapshot);
}
