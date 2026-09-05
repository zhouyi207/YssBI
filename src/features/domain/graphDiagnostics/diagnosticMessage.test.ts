import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";
import { formatGraphDiagnostic } from "./nodeDiagnostics";

describe("graph diagnostic messages", () => {
  it("localizes the same safe fact and falls back when the template contract is unavailable", () => {
    const diagnostic = {
      code: "compiler.input.unbound",
      messageKey: "diagnostics.compiler.input.unbound",
      arguments: { port: "Input" },
    };
    expect(formatGraphDiagnostic(diagnostic, "en-US")).toContain("Input");
    expect(formatGraphDiagnostic(diagnostic, "zh-CN")).toContain("输入");
    expect(formatGraphDiagnostic(diagnostic, "zh-CN")).not.toContain(diagnostic.code);
    expect(formatGraphDiagnostic({ ...diagnostic, arguments: {} }, "en-US")).toBe(
      "Graph problem (compiler.input.unbound).",
    );
    expect(formatGraphDiagnostic({ ...diagnostic, messageKey: "constructor" }, "zh-CN")).toBe(
      "图问题（compiler.input.unbound）。",
    );
  });

  it("keeps frontend templates generated from the Rust diagnostic vocabulary", () => {
    const result = spawnSync(
      process.execPath,
      ["scripts/generate-graph-diagnostics.mjs", "--check"],
      { encoding: "utf8" },
    );
    expect(result.status, result.stderr).toBe(0);
  });
});
