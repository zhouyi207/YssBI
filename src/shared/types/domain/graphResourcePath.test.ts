import { describe, expect, it } from 'vitest';
import {
  decodeGraphResourceKey,
  encodeGraphResourceKey,
  graphPathFromResourceUri,
  graphResourceUriFromPath,
  parseGraphResourceUri,
  parseUntitledGraphPath,
  buildUntitledGraphPath,
  inferGraphResourceKind,
  isValidGraphResourceTabId,
  toGraphResourceUri,
} from './graphResourcePath';

describe('graphResourcePath', () => {
  it('round-trips nested paths through encode/decode', () => {
    expect(encodeGraphResourceKey('functions/math/add')).toBe('functions::math::add');
    expect(decodeGraphResourceKey('functions::math::add')).toBe('functions/math/add');
  });

  it('round-trips graph resource URIs', () => {
    const uri = toGraphResourceUri('function', 'functions/My Fn');
    expect(uri).toBe('yssbi://graph/function/functions::My Fn');
    expect(parseGraphResourceUri(uri)).toEqual({
      kind: 'function',
      path: 'functions/My Fn',
    });
    expect(graphPathFromResourceUri(uri)).toBe('functions/My Fn');
    expect(graphResourceUriFromPath('event', 'events/Main')).toBe(
      'yssbi://graph/event/events::Main',
    );
  });

  it('rejects non-graph URIs', () => {
    expect(parseGraphResourceUri('file:///tmp/x')).toBeNull();
    expect(graphPathFromResourceUri('yssbi://graph/worksheet/x')).toBeNull();
  });

  it('parses untitled graph handles', () => {
    expect(parseUntitledGraphPath('untitled:event:Untitled-1')).toEqual({
      kind: 'event',
      label: 'Untitled-1',
    });
    expect(buildUntitledGraphPath('function', 'Untitled-2')).toBe(
      'untitled:function:Untitled-2',
    );
    expect(inferGraphResourceKind('untitled:event:Untitled-1')).toBe('event');
    expect(isValidGraphResourceTabId('untitled:event:Untitled-1', 'event')).toBe(true);
    expect(isValidGraphResourceTabId('untitled:event:Untitled-1', 'function')).toBe(false);
  });
});
