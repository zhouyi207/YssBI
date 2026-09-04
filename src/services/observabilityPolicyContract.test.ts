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
      (path) =>
        !path.startsWith("src/features/application/log/") &&
        !path.startsWith("src/features/application/observability/"),
    );
    const diagnosticImports =
      /(?:@\/features\/core\/log|@\/services\/log|@\/shared\/types\/dto\/diagnostics)/;

    expect(matchingFiles(applicationFiles, diagnosticImports)).toEqual([]);
  });

  it("keeps Problems and Run Output independent from operational diagnostics", () => {
    const problemsFiles = productionFiles("src/modules/problems", [".ts", ".tsx"]);
    const outputFiles = productionFiles("src/modules/output", [".ts", ".tsx"]);
    const logsPublic = readFileSync(resolve("src/modules/logs/public.ts"), "utf8");
    const problemsPublic = readFileSync(resolve("src/modules/problems/public.ts"), "utf8");
    const outputPublic = readFileSync(resolve("src/modules/output/public.ts"), "utf8");
    const operationalLogImports =
      /(?:features\/(?:application|core)\/log|services\/log)(?:\/|["'])/;
    const operationalDiagnosticIdentifiers =
      /\b(?:DiagnosticRecordDto|DiagnosticBatchDto|DiagnosticSubscriptionDto|GraphProjectionChannel|ProblemsChannel|useDiagnosticSubscription|useLiveLogs|useProblemsStore|logBuffer|submit_frontend_diagnostics|subscribe_diagnostics|unsubscribe_diagnostics)\b/;
    const tauriChannelImports = /@tauri-apps\/api\/(?:core|event)/;
    const graphProjectionImports =
      /@\/features\/(?:core\/(?:graph|dataStore\/graphProjectionStore)|domain\/editorProjection)(?:\/|["'])/;

    expect(
      [operationalLogImports, operationalDiagnosticIdentifiers, tauriChannelImports].flatMap(
        (pattern) => matchingFiles(problemsFiles, pattern),
      ),
    ).toEqual([]);
    expect(
      [operationalLogImports, operationalDiagnosticIdentifiers].flatMap((pattern) =>
        matchingFiles(outputFiles, pattern),
      ),
    ).toEqual([]);
    expect(matchingFiles(outputFiles, graphProjectionImports)).toEqual([]);
    expect(logsPublic).not.toMatch(/GraphProblemsPanel|RunOutputPanel/);
    expect(problemsPublic).toMatch(/GraphProblemsPanel/);
    expect(outputPublic).toMatch(/RunOutputPanel/);
  });

  it("keeps compiler problems separate from operational diagnostics", () => {
    const diagnosticsFiles = productionFiles("src-tauri/crates/yss-diagnostics/src", [".rs"]);
    const compilerDiagnosticsFiles = productionFiles(
      "src-tauri/crates/yss-graph-compiler-diagnostics/src",
      [".rs"],
    );
    const graphAnalysisFiles = productionFiles("src-tauri/crates/yss-graph-analysis/src", [".rs"]);
    const diagnosticsManifest = readFileSync(
      resolve("src-tauri/crates/yss-diagnostics/Cargo.toml"),
      "utf8",
    );
    const compilerDiagnosticsManifest = readFileSync(
      resolve("src-tauri/crates/yss-graph-compiler-diagnostics/Cargo.toml"),
      "utf8",
    );
    const graphAnalysisManifest = readFileSync(
      resolve("src-tauri/crates/yss-graph-analysis/Cargo.toml"),
      "utf8",
    );
    const ordinaryCompilerProblems =
      /(?:compiler\.input\.unbound|node\.input\.not_connected|schema\.column_missing|type\.mismatch|parameter\.invalid|dynamic_pin\.orphan)/;

    expect(diagnosticsManifest).not.toMatch(/yss-graph-compiler-diagnostics/);
    expect(compilerDiagnosticsManifest).not.toMatch(/yss-diagnostics/);
    expect(graphAnalysisManifest).not.toMatch(/yss-diagnostics|yss-tracing/);
    expect(matchingFiles(diagnosticsFiles, /yss_graph_compiler_diagnostics/)).toEqual([]);
    expect(matchingFiles(compilerDiagnosticsFiles, /yss_diagnostics/)).toEqual([]);
    expect(matchingFiles(graphAnalysisFiles, /(?:yss_diagnostics|tracing::)/)).toEqual([]);
    expect(matchingFiles(diagnosticsFiles, ordinaryCompilerProblems)).toEqual([]);
  });
});
