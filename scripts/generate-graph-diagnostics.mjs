import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const diagnosticSource = readFileSync(
  resolve(root, "src-tauri/crates/yss-graph-compiler-diagnostics/src/lib.rs"),
  "utf8",
);
const start = diagnosticSource.indexOf("define_compiler_diagnostics! {");
const source = diagnosticSource.slice(start, diagnosticSource.indexOf("#[derive", start));
const literal = '("(?:\\\\.|[^"\\\\])*")';
const definition = new RegExp(
  `\\w+ \\{([^}]*)\\} => \\{\\s*code: ${literal},\\s*message_key: ${literal},\\s*severity: (\\w+),\\s*blocking: (true|false),\\s*en: ${literal},\\s*zh: ${literal},?\\s*\\}`,
  "gu",
);
const messages = {};
for (const match of source.matchAll(definition)) {
  const [, parameters, code, key, , , en, zh] = match;
  const messageKey = JSON.parse(key);
  if (messageKey in messages) throw new Error(`Duplicate diagnostic template ${messageKey}`);
  messages[messageKey] = {
    code: JSON.parse(code),
    parameters: parameters
      .split(",")
      .map((value) => value.trim())
      .filter(Boolean),
    "en-US": JSON.parse(en),
    "zh-CN": JSON.parse(zh),
  };
}
const definitionCount = [...source.matchAll(/message_key: "diagnostics\.compiler\./gu)].length;
if (Object.keys(messages).length !== definitionCount) {
  throw new Error("Diagnostic generator did not parse every compiler definition");
}
const output = resolve(
  root,
  "src/features/domain/graphDiagnostics/diagnosticTemplates.generated.json",
);
const content = `${JSON.stringify(messages, null, 2)}\n`;
if (process.argv.includes("--check")) {
  if (JSON.stringify(JSON.parse(readFileSync(output, "utf8"))) !== JSON.stringify(messages)) {
    throw new Error("Graph diagnostic templates are stale; run pnpm generate:diagnostics");
  }
} else {
  writeFileSync(output, content);
}
