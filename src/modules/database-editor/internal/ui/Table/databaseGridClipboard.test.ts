import { describe, expect, it } from "vitest";
import { parseDatabaseGridClipboard } from "./databaseGridClipboard";

describe("database grid clipboard", () => {
  it("preserves literal quotes and safely parses quoted TSV fields", () => {
    expect(parseDatabaseGridClipboard('He said "hi"\tsecond')).toEqual([
      ['He said "hi"', "second"],
    ]);
    expect(parseDatabaseGridClipboard('"first\tcell"\t"line 1\nline 2"')).toEqual([
      ["first\tcell", "line 1\nline 2"],
    ]);
    expect(parseDatabaseGridClipboard('unfinished"quote\tsecond')).toEqual([
      ['unfinished"quote', "second"],
    ]);
    expect(parseDatabaseGridClipboard('"Hello" world\tsecond')).toEqual([
      ['"Hello" world', "second"],
    ]);
    expect(parseDatabaseGridClipboard('"abc"def\tsecond')).toEqual([['"abc"def', "second"]]);
    expect(parseDatabaseGridClipboard('"unfinished\tsecond')).toEqual([['"unfinished', "second"]]);
  });
});
