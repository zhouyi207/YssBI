import { readFileSync, readdirSync } from "node:fs";
import { extname, join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";

function productionFiles(directory: string, extensions: readonly string[]): string[] {
  const root = resolve(directory);
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = join(root, entry.name);
    if (entry.isDirectory()) return productionFiles(path, extensions);
    const normalized = relative(resolve("."), path).replace(/\\/g, "/");
    if (!extensions.includes(extname(path)) || /\.(?:test|spec)\.[^/]+$/.test(normalized)) {
      return [];
    }
    return [normalized];
  });
}

function matchingFiles(paths: readonly string[], pattern: RegExp): string[] {
  return paths.filter((path) => pattern.test(readFileSync(resolve(path), "utf8")));
}

describe("observability policy contract", () => {
  it("keeps removed logging stacks and disk pagination out of Rust production code", () => {
    const rustFiles = productionFiles("src-tauri/src", [".rs"]);
    const forbidden = [
      /\btauri_plugin_log\b/,
      /\bLogManager\b/,
      /\bcommand_log\b/,
      /\bget_logs\b/,
      /\bget_log_count\b/,
      /["']log-message["']/,
    ];
    const offenders = forbidden.flatMap((pattern) => matchingFiles(rustFiles, pattern));
    const cargo = readFileSync(resolve("src-tauri/Cargo.toml"), "utf8");

    expect([...new Set(offenders)]).toEqual([]);
    expect(cargo).not.toMatch(/tauri-plugin-log|tracing-appender/);
  });

  it("keeps business application workflows independent from diagnostic storage", () => {
    const applicationFiles = productionFiles("src/features/application", [".ts", ".tsx"]).filter(
      (path) => !path.startsWith("src/features/application/log/"),
    );
    const diagnosticImports =
      /(?:@\/features\/core\/log|@\/services\/log|@\/shared\/types\/dto\/diagnostics)/;

    expect(matchingFiles(applicationFiles, diagnosticImports)).toEqual([]);
  });
});
